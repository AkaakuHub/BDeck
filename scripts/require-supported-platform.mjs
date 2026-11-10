// Linux builds do not compile yet, so stop before Cargo with a clear note.
if (process.platform === "linux") {
  console.error(
    "BDeck supports macOS and Windows. Linux is not supported.",
  );
  process.exit(1);
}
