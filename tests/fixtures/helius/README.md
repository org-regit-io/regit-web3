# Helius source fixtures

- `parsed_events.json`: complete current official Parsed Events example from https://www.helius.dev/docs/parsed-events/parsed-response.md, retrieved 2026-10-07. Signature, exact source amounts, slot 433950192 and every raw/decoded instruction are preserved. This is documentation data, not a live library qualification.
- `asset.json`: adapted deterministic DAS example using public identities and documented source shapes from https://www.helius.dev/docs/api-reference/das/getasset and https://www.helius.dev/docs/api-reference/das/getassetsbyowner. Content display text/URI is synthetic; quantities, empty uncompressed hash sentinels and royalty units exercise source validation. It is not a fetched asset observation.
- Tests derive explicit precision/malformed/capacity/network/pagination cases from these fixtures. No signing, secrets, writes or inferred canonical transaction proof.
