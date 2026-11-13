# Potentials to look at later (notes only, nothing queued)

Researched 4 Nov 2026. Not in `WORK_QUEUE.md`. Revisit after the normal edition is finished.

## Nex-N2.5 (Nex-AGI): possible optional cloud brain
- Sizes: mini (35B, Apache-2.0), Pro, Max (1.6T MoE, text-only). Weights on Hugging Face; licence for Pro/Max not confirmed (check before commercial use).
- Not a local model: mini wants ~2x H100; quantised builds still need ~20 GB+. Cannot live inside Groovy (150 MB idle target). Cloud only.
- Hosted on OpenRouter (mini and Pro) per third-party coverage; **price not confirmed, may not be free**.
- "Beats GPT-5.6" is only partly true: Max leads BrowseComp, trails top closed models on coding; most scores are self-reported.
- Fit: another provider behind the `Brain` interface (OpenAI-compatible endpoint, user's own key, optional). Supports tool calling.
- Before adding: check real per-token price, test on Groovy's actual tasks, confirm licence.
- Sources: huggingface.co/nex-agi/Nex-N2.5-mini, mindstudio.ai Nex-N2.5 article.

## Oracle Cloud free tier: possible Studio backend
- Trial: US$300 credit, up to 30 days. Always Free after: roughly 2 Arm CPUs + 12 GB RAM, two 1 GB AMD VMs, 200 GB block storage (limits from a third-party 2026 guide; verify on Oracle's page).
- Catches: card + phone verification; home region fixed; Arm "out of capacity" errors; idle reclaim if <20% use for 7 days; idle accounts can be suspended; no way back to free-only after upgrading to pay-as-you-go.
- No GPU, so it cannot host big models. Possible uses for Studio only: always-on post scheduler, phone-link relay, small router.
- If ever used: London region, never put API keys in the repo, keep it replaceable.
- Source: docs.oracle.com/iaas/Content/FreeTier/freetier.htm.
