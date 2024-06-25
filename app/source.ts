import { map } from "@/.map"
import { createMDXSource } from "fumadocs-mdx"
import { loader } from "fumadocs-core/source"

export const { getPage, getPages, pageTree, files } = loader({
  baseUrl: "/docs",
  rootDir: "docs",
  source: createMDXSource(map)
})
