# Coverage

GitHub workflow `.github/workflows/coverage.yml` generates `coverage.svg` in
`badges` branch.

To generate badge first generate coverage report:

```sh
cargo llvm-cov --json --output-path coverage.json
```

Then run python script to generate badge SVG.

```sh
python3 scripts/coverage-badge.py coverage.json badges/coverage.svg
```
