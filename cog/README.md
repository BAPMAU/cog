# cog

Mémoire externe persistante pour skills et agents — une petite CLI Rust adossée à SQLite.

Un agent ou un skill perd son contexte d'une exécution à l'autre. `cog` lui donne
un état qui **survit entre les process** : des journaux d'événements et des machines
à états, accessibles via des commandes simples qui renvoient toujours du JSON.

## Installation

Le plus simple — installe `cog` sur le PATH depuis le dépôt (nécessite une toolchain Rust) :

```sh
cargo install --git https://github.com/BAPMAU/cog --locked
cog --help            # utilisable depuis n'importe quel dossier
```

Depuis un clone local (pour développer cog) :

```sh
cargo build --release   # binaire : target/release/cog
cargo install-local     # (ré)installe le clone dans ~/.cargo/bin
```

## Comment ça marche

Chaque commande lit ou écrit un état durable dans un fichier SQLite
(`.cog/state.db` par défaut ; `--store <path>` pour en choisir un autre) et
imprime un résultat JSON sur la sortie standard.

Deux briques :

- **`log`** — un flux *append-only* d'entrées JSON. On ajoute des entrées, on les
  relit du plus récent au plus ancien. Utile pour un historique d'événements.
- **`fsm`** — une machine à états décrite en JSON (états + transitions). On la
  définit une fois, puis on la fait avancer pas à pas ; les transitions illégales
  sont refusées.

## Commandes

```
cog [--store <path>] <command>

GLOBAL OPTIONS
    --store <path>    fichier de base (défaut : .cog/state.db)
    -h, --help        affiche l'aide

COMMANDS
    log add <stream> <json>        ajoute une entrée JSON à un flux
    log query <stream>             liste les entrées d'un flux, plus récentes d'abord
    fsm define <name> <def-json>   définit une machine à états depuis du JSON
    fsm transition <name> <state>  fait passer une machine dans un nouvel état
    fsm state <name>               affiche l'état courant d'une machine
```

## Exemples

Mettez le JSON entre **quotes simples** pour que le shell n'y touche pas :

```sh
cog log add reviews '{"comment": 42}'
cog log query reviews

cog fsm define watch '{"states":[{"name":"idle"}],"initial":"idle"}'
cog fsm state watch
```

## Sortie et codes de retour

Chaque commande imprime un résultat JSON :

```json
{"ok": true,  "value": ...}
{"ok": false, "error": {"kind": ..., "code": ...}}
```

Codes de sortie : `0` succès, `2` erreur métier (domaine), `70` erreur technique.

## Développement

Les tâches de dev sont des alias Cargo (voir `.cargo/config.toml`). Liste-les avec :

```sh
cargo --list          # affiche les alias du projet (install-local, t, lint…)
```

```sh
cargo t               # lance les tests
cargo lint            # clippy en mode strict
cargo install-local   # (ré)installe cog sur le PATH
./coverage.sh         # rapport de couverture (HTML dans tmp/coverage/)
```

## Pour aller plus loin

- `work.md` — journal de construction et détails d'implémentation.
- `CLAUDE.md` — conventions du projet.
