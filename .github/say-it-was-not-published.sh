set -eu

line="$WHAT: $TAG was not published -- $WHY. $WHERE."

printf '%s\n' "$line" >> "$GITHUB_STEP_SUMMARY"

if ! gh release view "$TAG" --json body -q .body > body.md; then
    echo "::warning::$line (and the release notes could not be reached to say so)"
    exit 0
fi

if grep -qF "$line" body.md; then
    exit 0
fi

printf '\n\n%s\n' "$line" >> body.md
gh release edit "$TAG" --notes-file body.md
