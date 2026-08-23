import { Monitor, Moon, Sun } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useTheme, type Theme } from "@/components/theme-provider";

const CYCLE: Record<Theme, Theme> = {
  dark: "light",
  light: "system",
  system: "dark",
};

export function ModeToggle() {
  const { theme, setTheme } = useTheme();
  const Icon = theme === "dark" ? Moon : theme === "light" ? Sun : Monitor;
  return (
    <Button variant="outline" size="icon" onClick={() => setTheme(CYCLE[theme])}>
      <Icon className="h-[1.2rem] w-[1.2rem]" />
      <span className="sr-only">
        Theme {theme}. Click for {CYCLE[theme]} theme
      </span>
    </Button>
  );
}
