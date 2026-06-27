# Instructions projet — cog

CLI Rust = mémoire externe persistante (SQLite) pour skills/agents.

## Conventions de code

- **Commentaires de code en anglais.** Tout commentaire (`//`, `//!`, doc) est rédigé en anglais. Les échanges avec l'utilisateur restent en français.
- Architecture hexagonale + DDD : `src/domain` (entités, invariants), `src/application` (ports + use cases), `src/infra` (adapters SQLite, CLI), `src/main.rs` (composition root).
- **Deux canaux d'erreur** : `DomainError` (métier, `kind:"domain"`, exit 2) vs `TechnicalError` (infra, `kind:"technical"`, exit 70). Jamais de panic métier.
- **Une transaction par commande**, ouverte au composition root (`BEGIN IMMEDIATE` + WAL), rollback par RAII.
- Sortie JSON : `{"ok":true,"value":...}` ou `{"ok":false,"error":{...}}`.

Journal de construction détaillé : `work.md`.

## Fichiers temporaires

Tout fichier temporaire (scratch, sorties intermédiaires, brouillons) doit être placé dans `./tmp` à la racine de ce projet. Ce dossier est gitignoré.
