# Signing configuration for Uninstall Many Programs
#
# tauri.conf.json already sets:
#   bundle.windows.digestAlgorithm = "sha256"
#   bundle.windows.timestampUrl = "http://timestamp.digicert.com"
#   bundle.windows.certificateThumbprint = null  (fill before release)
#   bundle.windows.nsis.installMode = "perMachine"
#
# Before a public release:
# 1. Obtain an Authenticode code-signing certificate
# 2. Import it into the Windows certificate store
# 3. Set certificateThumbprint in src-tauri/tauri.conf.json (no spaces)
# 4. Run: npm run build
#
# Or sign artifacts after build:
#   signtool sign /fd sha256 /tr http://timestamp.digicert.com /td sha256 /sha1 <THUMBPRINT> <path-to-exe-or-installer>
#
# NSIS output name example:
#   Uninstall Many Programs_0.1.0_x64-setup.exe
