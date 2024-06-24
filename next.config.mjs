import createMDX from "fumadocs-mdx/config"
import { remarkHeading, remarkImage, remarkStructure, rehypeCode } from "fumadocs-core/mdx-plugins"
import remarkYoutube from "remark-youtube"

const withMDX = createMDX({
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
