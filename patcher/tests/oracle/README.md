# Patcher equivalence oracle

Run from the repository root with Python 3. The C# and Rust executables must already be built. Each manifest is copied with its supporting files into a temporary directory, and each tool gets its own copy of the pristine PBO and its own state directory. Temporary output is deleted after each manifest, including on failure.

```powershell
python patcher/tests/oracle/oracle.py `
  --patches D:/Arma/modpack/patches `
  --specimens D:/Arma/uksf-patcher/testdata/specimens `
  --specimens B:/FloPerf/@uksf_dependencies/addons `
  --csharp D:/Arma/uksf-patcher/src/UksfPatcher/bin/Release/net10.0/win-x64/uksf-patcher.exe `
  --rust target/release/uksf-patcher.exe `
  --scratch C:/Users/Tim/.agent-scratch/patcher-port
```

Use `--manifest ctab_core.pbo.json` to run one manifest; the option can be repeated. The tool loads only top-level JSON manifests in each mod folder, just as the C# tool does. Nested JSON files are supporting assets, not independent manifests. The current patch tree has 12 top-level manifests and three nested JSON assets.

A pass requires identical PBO bytes and semantically identical state JSON and backup files, successful verification by both tools, and an up-to-date second apply with unchanged PBO bytes and modification time. Failures include the first differing PBO byte offset, when applicable; the corresponding entry is not yet identified. The program exits nonzero if any manifest fails. Comparing the C# executable with itself is a harness smoke test, not Rust equivalence evidence.
