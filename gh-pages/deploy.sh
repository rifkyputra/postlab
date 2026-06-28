#!/usr/bin/env bash
# Deploy built static site to gh-pages branch.
# Run: cd gh-pages && npm run build && bash deploy.sh
set -euo pipefail

cd "$(dirname "$0")"

if [ ! -d build ]; then
	echo "Run 'npm run build' first." >&2
	exit 1
fi

# Ensure build/ is a git repo pointing at the gh-pages branch
if [ ! -d build/.git ]; then
	cd build
	git init
	git checkout -b gh-pages
	git remote add origin "$(cd ../.. && git remote get-url origin)"
	cd ..
fi

cd build
touch .nojekyll
git add -A
git commit -m "deploy $(date -u +%Y-%m-%dT%H:%M:%SZ)" || true
git push -f origin gh-pages
echo "Deployed to gh-pages branch."
