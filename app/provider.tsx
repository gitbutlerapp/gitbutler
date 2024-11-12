"use client"

import { Root } from "@radix-ui/react-dialog"
import DefaultSearchDialog, {
  type DefaultSearchDialogProps
} from "fumadocs-ui/components/dialog/search-default"
import { RootProvider } from "fumadocs-ui/provider"

export function Provider({ children }: { readonly children: React.ReactNode }) {
  return (
    <RootProvider
      search={{
        SearchDialog
      }}
    >
      {children}
    </RootProvider>
  )
}

function SearchDialog(props: DefaultSearchDialogProps): React.ReactElement {
  return (
    <Root>
      <DefaultSearchDialog {...props} />
    </Root>
  )
}
