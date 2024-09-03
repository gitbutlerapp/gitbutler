import { defineConfig, defineDocs } from "fumadocs-mdx/config"
import remarkYoutube from "remark-youtube"
import { remarkHeading, remarkImage, remarkStructure, rehypeCode } from "fumadocs-core/mdx-plugins"

export const { docs, meta } = defineDocs({
  docs: {}
})

export default defineConfig({
  lastModifiedTime: "git",
  mdxOptions: {
    rehypeCodeOptions: {
      inline: "tailing-curly-colon",
      themes: {
        light: "catppuccin-latte",
        dark: "catppuccin-mocha"
      }
    },
    remarkPlugins: [remarkHeading, remarkImage, remarkStructure, remarkYoutube],
    rehypePlugins: (v) => [rehypeCode, ...v]
  }
})
