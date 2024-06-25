import createMDX from "fumadocs-mdx/config"
import remarkYoutube from "remark-youtube"
import { remarkHeading, remarkImage, remarkStructure, rehypeCode } from "fumadocs-core/mdx-plugins"

const withMDX = createMDX({
  buildSearchIndex: {
    filter: (path) => {
      return path.startsWith("docs")
    }
  },
  mdxOptions: {
    remarkPlugins: [remarkHeading, remarkImage, remarkStructure, remarkYoutube],
    rehypePlugins: [[rehypeCode]]
  }
})

/** @type {import('next').NextConfig} */
const config = {
  reactStrictMode: true,
  compress: true,
  swcMinify: true,
  cleanDistDir: true,
  images: {
    remotePatterns: [
      {
        hostname: "docs.gitbutler.com"
      }
    ]
  }
}

export default withMDX(config)
