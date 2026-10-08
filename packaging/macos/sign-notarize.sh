#!/usr/bin/env bash
# Developer ID signing + notarization. Runs only when the repository secrets exist
# (APPLE_CERT = base64 .p12, APPLE_CERT_PASSWORD, APPLE_SIGN_ID, APPLE_ID, APPLE_TEAM_ID, APPLE_APP_PASSWORD).
set -euo pipefail
APP=$1
KC=build.keychain
echo "$APPLE_CERT" | base64 --decode > cert.p12
security create-keychain -p tmp $KC && security default-keychain -s $KC && security unlock-keychain -p tmp $KC
security import cert.p12 -k $KC -P "$APPLE_CERT_PASSWORD" -T /usr/bin/codesign
security set-key-partition-list -S apple-tool:,apple: -s -k tmp $KC >/dev/null
codesign --force --deep --options runtime --timestamp -s "$APPLE_SIGN_ID" "$APP"
ditto -c -k --keepParent "$APP" notarize.zip
xcrun notarytool submit notarize.zip --apple-id "$APPLE_ID" --team-id "$APPLE_TEAM_ID" --password "$APPLE_APP_PASSWORD" --wait
xcrun stapler staple "$APP"
rm -f cert.p12 notarize.zip
