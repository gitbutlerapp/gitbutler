import Link from "next/link"

export default function NotFound() {
  return (
    <div className="grid w-full min-w-0 max-w-[var(--fd-page-width)] flex-col place-items-center md:transition-[max-width]">
      <div className="prose flex flex-col gap-4 lg:flex-row">
        <div className="flex flex-col justify-center">
          <h2 className="text-3xl font-bold">Page Not Found</h2>
          <p>
            Could not find requested resource, please
            <br />
            try again later or <Link href="/">return home</Link>.
          </p>
        </div>

        <img src="/img/markus-broke.svg" className="mx-auto" />
      </div>
    </div>
  )
}
