import { createContext, useContext, useState, type ReactNode } from "react";

type AddFolderContextValue = {
  open: boolean;
  setOpen: (v: boolean) => void;
};

const AddFolderContext = createContext<AddFolderContextValue | null>(null);

export function AddFolderProvider({ children }: { children: ReactNode }) {
  const [open, setOpen] = useState(false);
  return (
    <AddFolderContext.Provider value={{ open, setOpen }}>
      {children}
    </AddFolderContext.Provider>
  );
}

export function useAddFolder() {
  const ctx = useContext(AddFolderContext);
  if (!ctx) throw new Error("useAddFolder must be used inside AddFolderProvider");
  return ctx;
}
