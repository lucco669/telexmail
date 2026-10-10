#!/bin/sh
# Vercel's "Ignored Build Step" (vercel.json): exit 0 skips the build, exit 1 builds.
# Skip only when nothing in site/ changed since the last deployment. Vercel clones just the newest
# commits, so after a run of app commits the last deployed one may be missing from the clone: fetch
# that one commit, and if it still can't be found, build rather than guess.

prev="${VERCEL_GIT_PREVIOUS_SHA:-HEAD^}"

if ! git cat-file -e "$prev^{commit}" 2>/dev/null; then
  git fetch --quiet --depth=1 origin "$prev" 2>/dev/null
  if ! git cat-file -e "$prev^{commit}" 2>/dev/null; then
    echo "Last deployed commit $prev isn't in the clone: building."
    exit 1
  fi
fi

if git diff --quiet "$prev" HEAD -- .; then
  echo "Nothing in site/ changed since $prev: skipping the build."
  exit 0
fi
echo "site/ changed since $prev: building."
exit 1
