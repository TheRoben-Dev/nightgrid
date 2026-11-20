#!/bin/sh
# Creates Nightgrid's stable macOS code-signing certificate (D-193) and,
# when the GitHub CLI is signed in, stores it in the repo's Actions secrets.
#
# Why: ad-hoc signed builds get a new code signature with every release, so
# macOS forgets "Always Allow" on every keychain item after each update. CI
# signs each release with this one certificate instead, so updates stay the
# same app to the keychain. No Apple developer account is involved.
#
# Run once, on any machine with openssl (macOS's own is fine):
#   sh scripts/macos-signing-cert.sh
#
# The certificate and its password are kept in ~/.nightgrid-signing (outside
# the repo). Keep that folder: a new certificate means one more round of
# keychain prompts after the next update. The script never overwrites it.
set -eu

REPO="${NIGHTGRID_REPO:-TheRoben-Dev/nightgrid}"
DIR="${NIGHTGRID_SIGNING_DIR:-$HOME/.nightgrid-signing}"
NAME="Nightgrid Self-Signed Code Signing"

# macOS ships LibreSSL at /usr/bin/openssl; prefer it so `security import`
# reads the .p12 on every macOS version.
OPENSSL=openssl
if [ -x /usr/bin/openssl ]; then OPENSSL=/usr/bin/openssl; fi

if [ -e "$DIR/nightgrid-signing.p12" ]; then
  echo "A certificate already exists in $DIR; reusing it (never replaced)."
else
  umask 077
  mkdir -p "$DIR"
  cat > "$DIR/cert.cnf" <<CNF
[ req ]
distinguished_name = dn
prompt = no
x509_extensions = ext
[ dn ]
CN = $NAME
O = Nightgrid
[ ext ]
basicConstraints = critical, CA:false
keyUsage = critical, digitalSignature
extendedKeyUsage = critical, codeSigning
subjectKeyIdentifier = hash
CNF
  "$OPENSSL" req -x509 -newkey rsa:3072 -sha256 -nodes -days 7300 \
    -config "$DIR/cert.cnf" -keyout "$DIR/key.pem" -out "$DIR/cert.pem" 2>/dev/null
  "$OPENSSL" rand -hex 24 > "$DIR/p12-password.txt"
  # 3DES + SHA-1 MAC: the .p12 format every macOS `security import` accepts
  # (OpenSSL 3 would otherwise default to AES, which older macOS rejects).
  legacy=""
  if "$OPENSSL" version | grep -q '^OpenSSL 3'; then
    legacy="-keypbe PBE-SHA1-3DES -certpbe PBE-SHA1-3DES -macalg sha1"
  fi
  # shellcheck disable=SC2086
  "$OPENSSL" pkcs12 -export $legacy -name "$NAME" \
    -inkey "$DIR/key.pem" -in "$DIR/cert.pem" \
    -out "$DIR/nightgrid-signing.p12" -passout "file:$DIR/p12-password.txt"
  rm -f "$DIR/key.pem" "$DIR/cert.cnf"
  echo "Created $DIR/nightgrid-signing.p12 (valid 20 years)."
fi

fingerprint=$("$OPENSSL" x509 -in "$DIR/cert.pem" -noout -fingerprint -sha1 | sed 's/.*=//; s/://g')
echo "Certificate SHA-1: $fingerprint"

if command -v gh >/dev/null 2>&1 && gh auth status >/dev/null 2>&1; then
  base64 < "$DIR/nightgrid-signing.p12" | tr -d '\n' | gh secret set MACOS_SIGNING_CERT -R "$REPO"
  tr -d '\n' < "$DIR/p12-password.txt" | gh secret set MACOS_SIGNING_CERT_PASSWORD -R "$REPO"
  echo "Stored MACOS_SIGNING_CERT and MACOS_SIGNING_CERT_PASSWORD in $REPO's Actions secrets."
else
  echo
  echo "GitHub CLI isn't installed or signed in. Add two repository secrets by hand"
  echo "(github.com/$REPO → Settings → Secrets and variables → Actions → New repository secret):"
  echo "  MACOS_SIGNING_CERT           = paste after running: base64 -i \"$DIR/nightgrid-signing.p12\" | pbcopy"
  echo "  MACOS_SIGNING_CERT_PASSWORD  = contents of $DIR/p12-password.txt"
fi
