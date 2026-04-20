import { pushd, popd, writeLine } from "./lib.ts";

// Create a file with forbidden content
pushd("local-with-hooks");
writeLine("forbidden.txt", "This contains FORBIDDEN content");
popd();
