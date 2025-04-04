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
    remotePatterns: [
      {
        hostname: "docs.gitbutler.com"
      }
    ],
    localPatterns: [
      {
        pathname: "/img/**"
      }
    ]
  }
}

export default createMDX()(config)
