# Provider Adapters

## Contract

All adapters implement `ProviderAdapter`:

- `capabilities()` — advertised features
- `validate_and_translate()` — IR → provider request
- `generate()` — non-streaming
- `stream()` — streaming with cancellation
- `map_error()` — provider errors → canonical taxonomy

Provider API keys remain **gateway-side only**. Upstream URLs come from configuration. Automatic cross-host redirects are disabled unless explicitly validated. No retry after streaming begins.

## Capability matrix (MVP)

| Capability | OpenAI | Anthropic | Gemini | Mock |
|------------|--------|-----------|--------|------|
| Text chat | yes | yes | yes | yes |
| Streaming SSE | yes | yes | yes | yes |
| Tools / functions | yes | yes | yes | limited |
| Vision (bounded images) | yes | yes | yes | echo meta |
| JSON response format | yes | partial | partial | yes |
| Seed | yes | no* | partial | yes |

\* Unsupported fields yield `unsupported_capability` unless compatibility policy is `strip`.

## Semantic differences

- Anthropic uses `system` as a top-level field; Darial maps IR system messages accordingly.
- Gemini uses different role names and safety settings; refusals map to canonical `refusal`.
- Finish reasons are normalized to a small enum (`stop`, `length`, `tool_calls`, `content_filter`, `other`).

## Testing

Mock servers in `tests/` cover normal, streaming, tool, multimodal, refusal, quota, and malformed responses. Live tests are opt-in via `DARIAL_LIVE_TESTS=1` and cost-bounded; they MUST NOT send repository secrets.
