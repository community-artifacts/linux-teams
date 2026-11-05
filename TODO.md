# TODO

- [ ] **Short install URL.** Put the installer behind a public URL shortener so the README command is easy to type and share, e.g. `curl -fsSL https://<short-url> | bash` instead of the long `raw.githubusercontent.com/community-artifacts/linux-teams/main/install.sh`.
  - Pick a service that supports permanent redirects and edits (e.g. a custom domain, or a shortener with a stable alias), and point it at the raw `install.sh` URL.
  - Check that `curl -fsSL` follows the redirect and that the service doesn't show an interstitial page, which would break `| bash`.
  - Update the Install section of `README.md` (and the header comment in `install.sh`) with the short URL; keep the full URL as a fallback.
  - Consider pinning to a release tag instead of `main` so the installer script matches the AppImage version.
