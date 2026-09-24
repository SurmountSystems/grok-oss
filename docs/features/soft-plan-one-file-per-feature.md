# Each feature plan is its own file

Source: the same Operator box, plus the later instruction that there are more than two plans.

"I also worry it overwrote the last secondary plan... Use ULIDs please."

"There should be more than two plans, and they should be persisted as features in documents, and iterated over. Thoughtfully named."

There is not a primary slot and a secondary slot. Each feature has its own document in this project's `docs/features/` directory. The name says what the feature is. A revision edits that document. A new feature is a new document. A ULID keeps the file unique so a new plan does not erase an older one. The pane title is the document name.

The product today writes one `secondary-plan.md` and replaces it. That is the overwrite.

These four files are the documents:

- `docs/features/l2-shown-token-is-its-own-context-plus-its-l3s.md`
- `docs/features/soft-plan-implements-after-approve.md`
- `docs/features/soft-plan-one-file-per-feature.md`
- `docs/features/soft-plan-waits-for-the-button.md`

Status: the product still does not create them. Later notes go in the matching file.
