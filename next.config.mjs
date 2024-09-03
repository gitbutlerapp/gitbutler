import { createMDX } from "fumadocs-mdx/next"

/** @type {import('next').NextConfig} */
const config = {
  reactStrictMode: true,
  eslint: {
    ignoreDuringBuilds: true
  },
  compress: true,
  swcMinify: true,
  cleanDistDir: true,
  images: {
    unoptimized: true,
    remotePatterns: [
      {
        hostname: "docs.gitbutler.com"
      }
    ]
  }
}

export default createMDX()(config)
