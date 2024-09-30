import { ImageZoom } from "fumadocs-ui/components/image-zoom"

interface Props {
  width: number
  height: number
  src: string
  className?: string
  alt?: string
  subtitle?: string
}

export default function ImageSection({ width, height, className, alt, src, subtitle }: Props) {
  return (
    <div className="mx-auto mb-4 flex max-w-2xl flex-col justify-start rounded-lg border border-neutral-200 bg-neutral-100 p-2 dark:border-neutral-800 dark:bg-neutral-900 [&>span]:w-fit [&_img]:m-0">
      <ImageZoom
        width={width}
        height={height}
        className={`min-w-content mx-auto w-auto max-w-full rounded-md ${className ?? ""}`}
        alt={alt ?? ""}
        src={src}
      />
      {subtitle ? (
        <div className="mx-auto mt-2 flex-shrink whitespace-normal text-pretty break-words text-center text-sm opacity-50">
          {subtitle}
        </div>
      ) : null}
    </div>
  )
}
