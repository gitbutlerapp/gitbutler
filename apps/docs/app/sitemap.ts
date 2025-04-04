import type { MetadataRoute } from "next"
import { utils } from "./source"

const baseUrl =
  process.env.NODE_ENV === "development"
    ? new URL("http://localhost:3000")
    : new URL(`https://${process.env.VERCEL_URL}`)

export default function sitemap(): MetadataRoute.Sitemap {
  const url = (path: string): string => new URL(path, baseUrl).toString()

  return [
    {
      url: url("/"),
      changeFrequency: "monthly",
      priority: 0.8
    },
    ...utils.getPages().map<MetadataRoute.Sitemap[number]>((page) => ({
      url: url(page.url),
      lastModified: page.data.lastModified ? new Date(page.data.lastModified) : undefined,
      changeFrequency: "weekly",
      priority: 0.5
    }))
  ]
}
