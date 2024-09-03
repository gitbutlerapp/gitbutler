import { openapi, utils } from "@/app/source"
import { DocsPage, DocsBody, DocsTitle, DocsDescription } from "fumadocs-ui/page"
import { notFound } from "next/navigation"
import defaultComponents from "fumadocs-ui/mdx"
import { Popup, PopupContent, PopupTrigger } from "fumadocs-ui/twoslash/popup"
import { Tab, Tabs } from "fumadocs-ui/components/tabs"
import { Callout } from "fumadocs-ui/components/callout"
import { TypeTable } from "fumadocs-ui/components/type-table"
import { Accordion, Accordions } from "fumadocs-ui/components/accordion"
import ImageSection from "@/app/components/ImageSection"
import type { ComponentProps, FC } from "react"

interface Param {
  slug: string[]
}

export default function Page({ params }: { params: Param }): React.ReactElement {
  const page = utils.getPage(params.slug)

  if (!page) notFound()

  const footer = (
    <>
      <a
        href={`https://github.com/gitbutlerapp/gitbutler-docs/blob/main/content/docs/${page.file.path}`}
        target="_blank"
        rel="noreferrer noopener"
        className="group flex items-center justify-center gap-2 rounded-md border border-neutral-300/50 py-1 text-sm text-neutral-500 transition duration-300 hover:bg-neutral-100 dark:border-neutral-700 dark:bg-neutral-900 dark:text-neutral-400 dark:hover:bg-neutral-950"
      >
        <svg
          className="size-4 group-hover:animate-[var(--animation-shake-x)]"
          xmlns="http://www.w3.org/2000/svg"
          width="32"
          height="32"
          fill="currentColor"
          viewBox="0 0 256 256"
        >
          <path d="M208.31,75.68A59.78,59.78,0,0,0,202.93,28,8,8,0,0,0,196,24a59.75,59.75,0,0,0-48,24H124A59.75,59.75,0,0,0,76,24a8,8,0,0,0-6.93,4,59.78,59.78,0,0,0-5.38,47.68A58.14,58.14,0,0,0,56,104v8a56.06,56.06,0,0,0,48.44,55.47A39.8,39.8,0,0,0,96,192v8H72a24,24,0,0,1-24-24A40,40,0,0,0,8,136a8,8,0,0,0,0,16,24,24,0,0,1,24,24,40,40,0,0,0,40,40H96v16a8,8,0,0,0,16,0V192a24,24,0,0,1,48,0v40a8,8,0,0,0,16,0V192a39.8,39.8,0,0,0-8.44-24.53A56.06,56.06,0,0,0,216,112v-8A58.14,58.14,0,0,0,208.31,75.68ZM200,112a40,40,0,0,1-40,40H112a40,40,0,0,1-40-40v-8a41.74,41.74,0,0,1,6.9-22.48A8,8,0,0,0,80,73.83a43.81,43.81,0,0,1,.79-33.58,43.88,43.88,0,0,1,32.32,20.06A8,8,0,0,0,119.82,64h32.35a8,8,0,0,0,6.74-3.69,43.87,43.87,0,0,1,32.32-20.06A43.81,43.81,0,0,1,192,73.83a8.09,8.09,0,0,0,1,7.65A41.72,41.72,0,0,1,200,104Z"></path>
        </svg>
        Edit on GitHub
      </a>
      <a
        href={`https://github.com/gitbutlerapp/gitbutler-docs/issues/new?label=docs&title=Feedback+for+page+"${page.file.flattenedPath}"`}
        target="_blank"
        rel="noreferrer noopener"
        className="group flex items-center justify-center gap-2 rounded-md border border-neutral-300/50 py-1 text-sm text-neutral-500 transition duration-300 hover:bg-neutral-100 dark:border-neutral-700 dark:bg-neutral-900 dark:text-neutral-400 dark:hover:bg-neutral-950"
      >
        <svg
          className="size-4 transition duration-500 ease-[var(--ease-spring-3)] group-hover:animate-[var(--animation-bounce)]"
          xmlns="http://www.w3.org/2000/svg"
          viewBox="0 0 256 256"
        >
          <rect width="256" height="256" fill="none" />
          <path
            d="M32,104H80a0,0,0,0,1,0,0V208a0,0,0,0,1,0,0H32a8,8,0,0,1-8-8V112A8,8,0,0,1,32,104Z"
            fill="none"
            stroke="currentColor"
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth="16"
          />
          <path
            d="M80,104l40-80a32,32,0,0,1,32,32V80h64a16,16,0,0,1,15.87,18l-12,96A16,16,0,0,1,204,208H80"
            fill="none"
            stroke="currentColor"
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth="16"
          />
        </svg>
        Give us feedback
      </a>
    </>
  )

  return (
    <DocsPage
      toc={page.data.toc}
      full={page.data.full}
      tableOfContent={{
        style: "clerk",
        single: false,
        footer
      }}
      lastUpdate={page.data.lastModified}
      tableOfContentPopover={{
        footer
      }}
    >
      <DocsTitle>{page.data.title}</DocsTitle>
      <DocsDescription>{page.data.description}</DocsDescription>
      <DocsBody>
        <page.data.body
          components={{
            ...defaultComponents,
            Popup,
            PopupContent,
            PopupTrigger,
            Tabs,
            Tab,
            TypeTable,
            Accordion,
            Accordions,
            ImageSection,
            blockquote: Callout as unknown as FC<ComponentProps<"blockquote">>,
            APIPage: openapi.APIPage
          }}
        />
      </DocsBody>
    </DocsPage>
  )
}

export function generateStaticParams(): Param[] {
  return utils.getPages().map((page) => {
    return {
      slug: page.slugs
    }
  })
}

export function generateMetadata({ params }: { params: { slug?: string[] } }) {
  const page = utils.getPage(params.slug)

  if (!page) notFound()

  return {
    title: page.data.title,
    description: page.data.description
  }
}
