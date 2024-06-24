import type { MDXComponents } from "mdx/types"
import defaultComponents from "fumadocs-ui/mdx"
import ImageSection from "@/components/ImageSection"

export function useMDXComponents(components: MDXComponents): MDXComponents {
  return {
    ImageSection,
    ...defaultComponents,
    ...components
  }
}
