import { pushd, popd, writeLine } from "./lib.ts";

// Create a file with allowed content
pushd("local-with-hooks");
writeLine("allowed.txt", "This is allowed content");
popd();
