import { credentialCacheInit } from "./generated/graph/index.js";
import { initializeCredentialCache } from "./credentialCache.js";

initializeCredentialCache("graph", credentialCacheInit);
export * from "./generated/graph/index.js";
