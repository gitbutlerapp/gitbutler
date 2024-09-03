import { utils } from "@/app/source"
import { createSearchAPI } from "fumadocs-core/search/server"

export const { GET } = createSearchAPI("advanced", {
  indexes: utils.getPages().map((page) => {
    return {
      title: page.data.title,
      structuredData: page.data.structuredData,
      id: page.url,
      url: page.url
    }
  })
})
