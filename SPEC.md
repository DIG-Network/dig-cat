# dig-cat — Specification (bootstrap placeholder)

This is the v0.0.0 bootstrap placeholder. The complete **normative** specification — the contract an
independent reimplementation could be built against — lands with the CAT builder surface in
**v0.1.0** and will cover:

- **Invariants** (INV-1 no network, INV-2 no keys, INV-3 unsigned output, INV-4 chia-wallet-sdk
  byte-source-of-truth).
- The **public type surface** (CAT/asset types re-exported from chia-wallet-sdk; the dig-cat-owned
  unsigned-result types).
- Each **operation builder** — issuance (single-issuance / everything-with-signature / delegated
  TAILs), send (single + multi-recipient), melt, combine, split — with its exact emitted
  conditions/announcements and the signatures the consumer must produce.
- **CAT coin selection** (checked sums, deterministic order, cap).
- **Lineage-proof reconstruction / hydration** from parent spends (fail-closed).
- **Multi-TAIL safety** invariants (per-TAIL ring separation; no cross-TAIL value creation; no
  untracked CAT leakage).
- The **signing boundary** (`required_signatures`) and the **error taxonomy**.
- **Security properties** and **conformance / golden-vector** notes.
