#!/usr/bin/env bash
#
# Assemble the published documentation site from git tags.
#
#   /            the newest release
#   /dev/        the current main
#   /vX.Y.Z/     every release whose docs still build
#
# The whole site is a pure function of the repository: nothing is carried
# forward from the previously published site, and no `gh-pages` branch holds
# state that can drift from the tags. Re-running this on the same commit
# produces the same site, which is the same property the renderer guarantees
# for pixels.
#
# A release whose docs no longer build is skipped with a warning rather than
# failing the deploy. Old documentation that cannot be rebuilt is a fact about
# the past, and losing today's deploy over it helps nobody.

set -euo pipefail

out="${1:-site}"
root="$(git rev-parse --show-toplevel)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

rm -rf "$out"
mkdir -p "$out"

# Newest first, so the first that builds becomes the site root.
mapfile -t tags < <(git tag -l 'v*' | sort -Vr)

built=()
for tag in "${tags[@]}"; do
  src="$work/$tag"
  mkdir -p "$src"
  if ! git archive "$tag" docs 2>/dev/null | tar x -C "$src" 2>/dev/null; then
    echo "  skip $tag — no docs/ at that tag"
    continue
  fi
  [ -d "$src/docs" ] || { echo "  skip $tag — no docs/ at that tag"; continue; }
  if mdbook build "$src/docs" >/dev/null 2>&1; then
    mkdir -p "$out/$tag"
    cp -r "$src/docs/book/." "$out/$tag/"
    built+=("$tag")
    echo "  built $tag"
  else
    echo "  skip $tag — its docs no longer build"
  fi
done

# `main`, under /dev/. Named for what it is: ahead of every release, and not
# what a reader landing on the site should get by default.
mdbook build "$root/docs" >/dev/null 2>&1
mkdir -p "$out/dev"
cp -r "$root/docs/book/." "$out/dev/"
echo "  built dev (main)"

# Inject the picker into every built version, using the *current* assets.
#
# It cannot come from each tag's own `book.toml`: a release tagged before the
# picker existed has no reference to it, so building that tag produces pages
# with no way to navigate away from an old version — which is precisely the
# version a reader is most likely to be stranded on. Injecting afterwards also
# means improving the picker improves every published version at once, rather
# than only the ones tagged since.
inject() {
  local dir="$1"
  cp "$root/docs/theme/version-picker.js" "$dir/version-picker.js"
  cp "$root/docs/theme/version-picker.css" "$dir/version-picker.css"
  python3 - "$dir" <<'PYEOF'
import pathlib, sys
root = pathlib.Path(sys.argv[1])
link = '<link rel="stylesheet" href="{p}version-picker.css">'
script = '<script src="{p}version-picker.js"></script>'
for html in root.rglob("*.html"):
    text = html.read_text(encoding="utf-8", errors="ignore")
    if "version-picker.js" in text:
        continue
    # Relative prefix back to this version's root, so a page nested in a
    # subdirectory still resolves the asset.
    depth = len(html.relative_to(root).parts) - 1
    prefix = "../" * depth
    if "</head>" in text:
        text = text.replace("</head>", link.format(p=prefix) + "\n</head>", 1)
    if "</body>" in text:
        text = text.replace("</body>", script.format(p=prefix) + "\n</body>", 1)
    html.write_text(text, encoding="utf-8")
PYEOF
}

for v in "${built[@]}"; do inject "$out/$v"; done
inject "$out/dev"
echo "  picker injected into ${#built[@]} release(s) and dev"

# The newest release is the root, so a bare link lands on documentation that
# describes something installable rather than something unreleased.
if [ ${#built[@]} -gt 0 ]; then
  cp -r "$out/${built[0]}/." "$out/"
  echo "  root = ${built[0]}"
else
  cp -r "$out/dev/." "$out/"
  echo "  root = dev (no release built)"
fi

# A machine-readable list, so a version picker never has to be edited by hand
# to stay in step with the tags.
{
  printf '{\n  "latest": "%s",\n  "versions": [' "${built[0]:-dev}"
  printf '"dev"'
  for t in "${built[@]}"; do printf ', "%s"' "$t"; done
  printf ']\n}\n'
} > "$out/versions.json"

echo "site assembled in $out/"
