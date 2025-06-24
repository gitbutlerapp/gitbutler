"use client"

import { MutableRefObject, ReactElement, useEffect, useId, useRef, useState } from "react"
import { MermaidConfig } from "mermaid"

function useIsVisible(ref: MutableRefObject<HTMLElement>) {
  const [isIntersecting, setIsIntersecting] = useState(false)

  useEffect(() => {
    const observer = new IntersectionObserver(([entry]) => {
      if (entry.isIntersecting) {
        // disconnect after once visible to avoid re-rendering of chart when `isIntersecting` will
        // be changed to true/false
        observer.disconnect()
        setIsIntersecting(true)
      }
    })

    observer.observe(ref.current)
    return () => {
      observer.disconnect()
    }
  }, [ref])

  return isIntersecting
}

export function Mermaid({ chart }: { chart: string }): ReactElement {
  const id = useId()
  const [svg, setSvg] = useState("")
  const containerRef = useRef<HTMLDivElement>(null!)
  const isVisible = useIsVisible(containerRef)

  useEffect(() => {
    // Fix when inside element with `display: hidden` https://github.com/shuding/nextra/issues/3291
    if (!isVisible) {
      return
    }
    const htmlElement = document.documentElement
    const observer = new MutationObserver(renderChart)
    observer.observe(htmlElement, { attributes: true })
    renderChart()

    return () => {
      observer.disconnect()
    }

    // Switching themes taken from https://github.com/mermaid-js/mermaid/blob/1b40f552b20df4ab99a986dd58c9d254b3bfd7bc/packages/mermaid/src/docs/.vitepress/theme/Mermaid.vue#L53
    async function renderChart() {
      const isDarkTheme =
        htmlElement.classList.contains("dark") ||
        htmlElement.attributes.getNamedItem("data-theme")?.value === "dark"

      const mermaidConfig: MermaidConfig = {
        securityLevel: "loose",
        fontFamily: "inherit",
        themeCSS: "margin: 1.5rem auto 0;",
        theme: isDarkTheme ? "dark" : "default",
        themeVariables: {
          background: isDarkTheme ? "#97eae5" : "#003366",
          primaryColor: isDarkTheme ? "#97eae5" : "#003366",
          git0: "#97eae5",
          git1: "#DC606B",
          git2: "#DC9B14"
        }
      }

      const { default: mermaid } = await import("mermaid")

      try {
        mermaid.initialize(mermaidConfig)
        const { svg } = await mermaid.render(
          // strip invalid characters for `id` attribute
          id.replaceAll(":", ""),
          chart.replaceAll("\\n", "\n"),
          containerRef.current
        )
        setSvg(svg)
      } catch (error) {
        console.error("Error while rendering mermaid", error)
      }
    }
  }, [chart, isVisible])

  return <div ref={containerRef} dangerouslySetInnerHTML={{ __html: svg }} />
}
