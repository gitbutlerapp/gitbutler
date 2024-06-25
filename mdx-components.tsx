import defaultComponents from "fumadocs-ui/mdx"
import ImageSection from "@/components/ImageSection"
import CodeEditor from "@/components/CodeEditor"
import type { MDXComponents } from "mdx/types"

export function useMDXComponents(components: MDXComponents): MDXComponents {
  return {
    ImageSection,
    CodeEditor,
    ...defaultComponents,
    ...components
  }
}
