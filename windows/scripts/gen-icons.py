# Round 4E item 4: the icon sizes Tauri ships, from the 1024 px master that
# scripts/gen-icons.mjs draws.  python3 scripts/gen-icons.py [master]
import sys
from PIL import Image
src = sys.argv[1] if len(sys.argv) > 1 else "src-tauri/icons/icon-1024.png"
im = Image.open(src).convert("RGBA")
d = "src-tauri/icons/"
for name, size in [("32x32.png", 32), ("128x128.png", 128), ("128x128@2x.png", 256), ("icon.png", 512)]:
    im.resize((size, size), Image.LANCZOS).save(d + name)
im.save(d + "icon.ico", sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
print("icons written")
