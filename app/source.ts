import { meta, docs } from "@/.source"
import { createMDXSource } from "fumadocs-mdx"
import { loader } from "fumadocs-core/source"
import { attachFile, createOpenAPI } from "fumadocs-openapi/server"
import type { InferMetaType, InferPageType } from "fumadocs-core/source"

export const utils = loader({
  source: createMDXSource(docs, meta),
  pageTree: {
    attachFile
  }
})

export const openapi = createOpenAPI({})

export type Page = InferPageType<typeof utils>
export type Meta = InferMetaType<typeof utils>
