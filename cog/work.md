# work.md — journal de construction `cog`

CLI Rust = mémoire externe persistante (SQLite) pour skills/agents.
Repart de zéro dans `/Users/baptou/dev/tools/cog` (l'ancien `/Users/baptou/cog` a disparu).
Design figé en mémoire `projet-cog` : hexagonal+DDD, 2 canaux d'erreur, transaction par commande.

## Décisions de design (ne pas re-débattre)
- 2 canaux d'erreur : `DomainError` (kind:"domain", exit 2) vs `TechnicalError` (kind:"technical", exit 70). Jamais de panic métier.
- Transaction par commande, ouverte au composition root (`main.rs`), RAII rollback, `BEGIN IMMEDIATE`+WAL+busy_timeout.
- Ports asymétriques : outbound = 1 trait/entité ; inbound = le use case EST l'API.
- Entités : `StateMachine` ouverte (def en données, invariant runtime) ; `Ledger`/`Cursor`/`SeenSet` fermées (invariant par le type).
- Sortie : JSON `{"ok":true,"value":...}` ou `{"ok":false,"error":{"kind":...,"code":...}}`.

## Périmètre
kv (socle) + Ledger + StateMachine + SeenSet/Cursor. Queue = phase 2.

## Journal

### Itération 1 — scaffold + vertical Ledger  [FAIT, vérifié exécution]
- [x] cargo init, arbo src/domain|application|infra
- [x] error.rs, domain/ledger.rs, ports.rs, usecases.rs
- [x] infra/db.rs, sqlite_ledger.rs, cli.rs, main.rs
- [x] vérif exécution : add(seq1/2), query(récent d'abord), empty→exit2, bad-json→exit70
- [x] FEEDBACK appliqué : commentaires de code en anglais (noté dans CLAUDE.md + mémoire)
- Dette : champ `at` en epoch millis (string), pas ISO. Horloge injectée plus tard.

### Itération 2 — entité StateMachine  [FAIT, vérifié exécution]
Le cas dur : def = donnée (pas enum codé en dur), invariant runtime, use case read-modify-write,
exerce les DEUX canaux (transition illégale = domaine ; load/save = technique).
- [x] error.rs : IllegalTransition/UnknownState/NotInitialized + codes
- [x] domain/state_machine.rs : Definition{states,transitions,terminal,initial}.validate() + transition(self,to) (consomme self)
- [x] application/ports.rs : trait StateStore { load, save } (def+current ensemble)
- [x] application/usecases.rs : DefineMachine, Transition (RMW), GetState
- [x] infra/sqlite_state.rs (upsert ON CONFLICT) + migration table state_machine
- [x] infra/cli.rs : fsm define|transition|state ; main.rs dispatch (chaque adapter sur la même &tx)
- [x] vérif 9 cas : define, state, transitions légales, illegal_transition(exit2),
      unknown_state(exit2), not_initialized(exit2), terminal bloque les sorties.
      Persistance vérifiée ENTRE process distincts (la propriété qui justifie le projet).
- [x] FEEDBACK appliqué : State et Transition sont des STRUCTURES, pas des strings.
      `State{name, terminal:bool, description?}` (terminal replié dans l'état, plus de Vec parallèle) ;
      `Transition{from, to, criterion?, description?}` (contexte d'arête optionnel).
      Champs de contexte optionnels (serde default + skip_if_none). Round-trip vérifié.

### Tests d'intégration end-to-end  [FAIT — `cargo test`]
`tests/cli.rs` : lance le BINAIRE compilé (`CARGO_BIN_EXE_cog`) contre un store temp
(`CARGO_TARGET_TMPDIR`), asserte stdout JSON + exit code. Couvre toute la composition.
- 10 tests verts : Ledger (seq monotone, ordre récent-d'abord, empty→domain/2, badjson→technical/70) ;
  FSM (define/state, transition légale + persistance inter-process, illegal/unknown/not_initialized/terminal → domain/2).
- Helper `run()` -> (code, serde_json::Value) ; `store_path(name)` isole + nettoie wal/shm.

### Coverage  [FAIT — `./coverage.sh`]
rustc `-C instrument-coverage` + outils Homebrew llvm (21.x, doit matcher la majeure LLVM de rustc ;
Xcode=17 trop vieux). Sortie : résumé terminal + HTML dans `tmp/coverage/html` (gitignoré).
- TOTAL : 94% lignes, 87% régions. domain/ledger 100%, state_machine 95%.
- PIÈGE résolu : les tests lancent `cog` en sous-process → c'est le BINAIRE `cog` qui porte les
  compteurs, à passer en `--object` (pas l'exécutable de test). Filtre cargo json sur kind=="bin".
- Manques = branches d'erreur non déclenchées (flag sans valeur, def corrompue en base, Display).

### Aide / usage  [FAIT]
`cog` sans arg / `-h` / `--help` → aide TEXTE lisible (pas JSON), exit 0 (intercepté dans main
avant le pipeline DB). Commande réellement erronée → erreur JSON propre ("no command given" vs
"unknown command 'x y'") pointant vers `cog --help`. `cli::USAGE` + `cli::wants_help()`.
3 tests e2e ajoutés (runner brut `run_raw` pour la sortie non-JSON). Total : 13 tests verts.

### Installation + ergonomie shell  [FAIT]
- `cargo install --path .` → `cog` dans `~/.cargo/bin` (déjà dans le PATH). ATTENTION : c'est un
  SNAPSHOT release ; après modif du code il faut RE-`cargo install` pour mettre à jour le `cog` global.
- Aide enrichie : section EXAMPLES avec JSON en guillemets SIMPLES (piège shell zsh : `{...}` non quoté
  → brace expansion / parse error). Note explicite dans l'aide.
- Confirmé : `cog log add/query` depuis un dossier neuf, store par défaut `.cog/state.db` (cwd).

### Arbre d'aide + canal Usage  [FAIT]
3e canal SÉPARÉ du JSON : `cli::Usage { Help(&str) | Error{sentence, help} }`.
- `-h`/`--help` à n'importe quel niveau → aide TEXTE scopée (HELP_TOP/HELP_LOG/HELP_FSM), stdout, exit 0.
- arg/sous-cmd manquant ou commande inconnue → 1 phrase + aide contextuelle, STDERR, exit 64 (EX_USAGE).
- stdout reste du JSON PUR (résultats uniquement) → propre pour les agents.
- main : `parse` sorti de `run` ; `run(Invocation)`. Exit codes : 0 / 2 domain / 64 usage / 70 technical.
- 16 tests verts (run_raw capture stdout+stderr). Binaire global réinstallé.

### Itération 3 — Context de FSM (dédup poll)  [FAIT — TDD, `cargo test`]
DÉCISION (ADR 0001) : on abandonne les entités fermées `Cursor`/`SeenSet`. À la place, la
StateMachine porte un `Context` — un petit blob JSON mutable, avancé dans la MÊME transaction
que la transition. Le curseur de poll d'un consommateur (high-water marks) vit dans ce Context.
Raison : phase + position doivent bouger atomiquement (1 commande = 1 transaction).
Les ids GitHub étant monotones, un high-water mark suffit — pas besoin d'un SeenSet.
Premier consommateur : le harness watch-pr (FSM de phase + curseur dans le Context + ledger).

Construit en TDD (tranches verticales, un test → une impl). 5 comportements verrouillés
(tests/cli.rs, total 21 verts) :
- `define --context '<json>'` stocke le blob ; `state` renvoie `{current, context}`.
- `transition --context '<json>'` avance l'état ET remplace le blob, atomiquement (vérifié
  inter-process).
- `transition` SANS `--context` préserve le blob (l'état bouge, le curseur reste).
- `define` sans `--context` → context `null` (pas d'état "context non initialisé" séparé).
- `--context` au JSON invalide → erreur technique (exit 70), comme une def JSON invalide.

Sémantique : blob OPAQUE (cog ne valide jamais sa forme), remplacement INTÉGRAL.
Couches touchées : `StateMachine.context: serde_json::Value` (define/rehydrate/transition le
portent) ; migration `state_machine` (+colonne `context`, `ALTER TABLE` gardé pour bases
existantes) ; sqlite_state load/save ; use cases `DefineMachine`(context) + `Transition`(Option)
+ `GetState`→`(current, context)` ; cli flag `--context` (valeur) ; main `parse_context`.

### Skill watch-pr (consommateur)  [FAIT — non testé en live]
`cog/skills/watch-pr/SKILL.md` (<100 l.) + `REFERENCE.md` écrits : Durable Harness (ADR 0002)
au-dessus de la feature Context. FSM `triage → fix_ci|handle_comments|await_review → triage`
(+`escalated`/`done`), curseur de poll dans le Context, Workers jetables + Ledger, escalade à
arrêt franc. Définition FSM + transition `--context` vérifiées de bout en bout sur le binaire.
Commandes `gh` reprises de watch-pull-request. Skill `cog` (SKILL+REFERENCE) aligné sur `--context`.

### Itération 4 — `cog inspect` (aperçu du store)  [FAIT — `cargo test`]
Manque comblé : aucune commande ne permettait de DÉCOUVRIR le contenu d'un store (`log query`
et `fsm state` exigent le nom à l'avance). `cog inspect [--name <substr>]` liste, en lecture
seule, un RÉSUMÉ de chaque entité.
- machine : `{name, current, terminal, has_context}` ; stream : `{name, count, last_seq, last_at}`.
- Sortie : `{"machines":[...],"streams":[...]}`, triées par nom. `--name` filtre les deux familles
  par sous-chaîne (`name.contains`, dans le use case).
- Streams résumés par AGRÉGAT SQL (`COUNT/MAX(seq)/MAX(at_millis) GROUP BY stream`) — aucune
  entrée chargée. Machines via `StateStore::list()` (rehydrate + `is_terminal()`).
- Décisions utilisateur : résumé seul (pas de context complet ni dernières entrées) ; un seul
  filtre `--name` (pas de `--kind`/`--state`).
- Couches : domain `StreamSummary` + `StateMachine::is_terminal()` pub ; ports
  `stream_summaries()`/`list()` ; use case `Inspect{ledger,state}` (read-model `Overview`/
  `MachineSummary`) ; adapters sqlite ; cli flag `--name` + `HELP_INSPECT` ; main dispatch.
- 4 tests e2e ajoutés (total 25 verts) : store vide, résumé multi-stream+fsm, filtre `--name`,
  état terminal + context absent. clippy clean.

## Note pour reprise
Reste à faire : (1) `cargo install --path .` — le `cog` global est un snapshot d'avant Context ;
le skill watch-pr invoque le `cog` du PATH. (2) Commiter (feature + docs design + skill).
(3) Test live de watch-pr sur une vraie PR. Push sur `main` bloqué par policy → demander/branche.
