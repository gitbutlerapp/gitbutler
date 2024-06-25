type EditorProps = {
  title?: string
  children: React.ReactNode
}

const CodeEditor = ({ title, children }: EditorProps) => {
  return (
    <div className="w-full">
      <div className="flex relative justify-between items-center -mb-4 w-full h-9 rounded-t-md dark:bg-neutral-800 bg-neutral-200">
        <div className="inline-flex justify-center h-full">
          <div className="inline-flex items-center ml-4 space-x-2 h-full">
            <span className="w-3 h-3 rounded-full bg-red-400"></span>
            <span className="w-3 h-3 rounded-full bg-yellow-400/80"></span>
            <span className="w-3 h-3 rounded-full bg-[var(--clr-accent)]"></span>
          </div>
          {title ? (
            <div className="inline-flex items-center p-2 ml-6 rounded-t-lg">
              <div className="font-mono text-sm font-normal text-gray-700 whitespace-normal break-all dark:text-gray-100">
                {title}
              </div>
            </div>
          ) : null}
        </div>
      </div>
      <div className="z-10 code-wrapper [&_figure]:!mt-4 [&_pre]:outline-none [&_figure]:!rounded-t-none">
        {children}
      </div>
    </div>
  )
}

export default CodeEditor
