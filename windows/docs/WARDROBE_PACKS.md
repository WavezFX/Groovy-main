# Wardrobe packs

A wardrobe pack is a JSON file of **presets**: combinations of Groovy's
built-in looks, outfits, accessories and colours. Import one in
**Settings → Wardrobe → Presets → Import a pack…**; **Export presets** writes
your own to `%LOCALAPPDATA%\Groovy\exports\`.

A pack names built-in items and colours only. It cannot contain images,
scripts, fonts, URLs or anything else that is loaded or run — Groovy's outfits
are drawn in code (`src/mochi/outfits.ts`), so a pack can't change how they
look, only which ones are worn and in which colours. That keeps imported packs
harmless: the worst a bad pack can do is be refused.

## Format (version 1)

```json
{
  "groovyWardrobePack": 1,
  "name": "Winter",
  "presets": [
    {
      "name": "Cosy",
      "look": "frost",
      "outfit": "beanie",
      "appearance": {
        "body": "#f2f6ff",
        "body2": "",
        "glow": "#7cc4ff",
        "eyes": "",
        "layers": ["leafScarf", "roundSpecs"]
      }
    }
  ]
}
```

| Field | Values |
| --- | --- |
| `groovyWardrobePack` | must be `1` |
| `presets[].name` | 1–40 characters (control characters are removed); a preset without a name is skipped |
| `presets[].look` | `night`, `ember`, `frost`, `grove`, `blaze`, `tide` — anything else becomes `night` |
| `presets[].outfit` | packs from before round 3 only: read as one more layer (below); new packs write `none` |
| `appearance.body`, `body2` | body gradient top and bottom, `#RRGGBB`, or `""` for the look's own |
| `appearance.glow`, `eyes` | `#RRGGBB` or `""` |
| `appearance.layers` | the items worn, at most one per slot (ids below) |

## Compatibility rules

Every item has a slot (src/mochi/wardrobe.ts):

- **head:** `leafCrown`, `acornCap`, `mushroomCap`, `flowerCrown`, `bucketHat`,
  `backwardsCap`, `headphones`, `pilotGoggles`, `beretPin`, `miniTopHat`,
  `cloudBuddy`, `antenna`, `diamondCrown`, and the seasonal `pumpkinCap`,
  `batWingBand`, `bobbleBeanie`, `twigAntlers`, `bunnyEarBand`, `eggshellHat`,
  `strawSunHat`, `watermelonCap`;
- **face:** `roundSpecs`, `shades`, `heartGlasses`, `glowVisor`, `monocle`,
  `frecklesBlush`, `starStickers`, `piratePatch`;
- **neck:** `bandana`, `bowTie`, `leafScarf`, `seedPendant`, `neckPhones`,
  `goldChain`, `flowerLei`, `necktie`, and the seasonal `candyCornScarf`,
  `littleGhostPal`, `knitStripeScarf`, `jingleBellCollar`, `pastelRibbonBow`,
  `daisyChain`, `shellNecklace`, `poolRing`.

- One item per slot; the first one named wins its slot.
- Ids of the old Mochi outfits are read as their nearest new item
  (`beanie` → `bucketHat`, `bow` → `bowTie`, `sunglasses` → `shades`,
  `roundGlasses` → `roundSpecs`, `scarf` → `leafScarf`); the others (party hat,
  crown, witch hat, Santa hat, bunny ears, pumpkin, auto) are dropped.
- Unknown items are dropped.
- A pack can name an item that waits for a stage or a season: it is kept in
  the preset, and worn only once he can wear it.

## Validation

Every import goes through `readPack()` in `src/mochi/appearance.ts`:

1. Files over 1 MB are refused.
2. The file must be JSON with `"groovyWardrobePack": 1`.
3. At most 50 presets are read; each one is cleaned field by field as above.
4. The person importing sees how many presets were imported and why any were
   skipped. Imported presets with the same name replace existing ones.

The same cleaning runs every time Groovy reads its own `settings.json`, so a
hand-edited or damaged file can't break the wardrobe either.
