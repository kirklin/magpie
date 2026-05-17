#!/bin/bash
# Generate a self-signed code signing certificate for Magpie
# This certificate is used to maintain a consistent signing identity
# across builds, so macOS preserves accessibility permissions on upgrade.
#
# Usage: ./scripts/generate-cert.sh
# Output: magpie-codesign.p12 (import this to GitHub Secrets)

set -euo pipefail

CERT_NAME="Magpie Open Source"
CERT_DAYS=3650  # 10 years
CERT_PASSWORD="magpie"
OUT_DIR="$(mktemp -d)"

echo "🔑 Generating self-signed code signing certificate..."
echo "   Name: ${CERT_NAME}"
echo "   Valid for: ${CERT_DAYS} days"
echo ""

# Generate private key
openssl genrsa -out "${OUT_DIR}/key.pem" 2048 2>/dev/null

# Create certificate with Code Signing extension
openssl req -new -x509 \
  -key "${OUT_DIR}/key.pem" \
  -out "${OUT_DIR}/cert.pem" \
  -days "${CERT_DAYS}" \
  -subj "/CN=${CERT_NAME}/O=Magpie" \
  -addext "keyUsage=critical,digitalSignature" \
  -addext "extendedKeyUsage=codeSigning"

# Package as .p12 (PKCS12)
openssl pkcs12 -export \
  -out magpie-codesign.p12 \
  -inkey "${OUT_DIR}/key.pem" \
  -in "${OUT_DIR}/cert.pem" \
  -passout "pass:${CERT_PASSWORD}"

# Clean up temp files
rm -rf "${OUT_DIR}"

echo "✅ Certificate generated: magpie-codesign.p12"
echo "   Password: ${CERT_PASSWORD}"
echo ""
echo "📋 Next steps:"
echo ""
echo "1. Base64 encode the certificate:"
echo "   base64 -i magpie-codesign.p12 | pbcopy"
echo ""
echo "2. Add GitHub Secrets (Settings → Secrets → Actions):"
echo "   APPLE_CERTIFICATE         = (paste the base64 from step 1)"
echo "   APPLE_CERTIFICATE_PASSWORD = ${CERT_PASSWORD}"
echo ""
echo "3. Delete the .p12 file (it's now in GitHub Secrets):"
echo "   rm magpie-codesign.p12"
echo ""
echo "⚠️  Keep this certificate safe. If you regenerate a new one,"
echo "   users will need to re-grant accessibility permissions once."
