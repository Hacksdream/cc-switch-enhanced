import { useTranslation } from "react-i18next";
import { open } from "@tauri-apps/plugin-dialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Button } from "@/components/ui/button";
import { FolderOpen, Trash2 } from "lucide-react";
import { isMac, isWindows, isLinux } from "@/lib/platform";

// Custom terminal type
export interface CustomTerminal {
  value: string;
  label: string;
}

// Terminal options per platform
const MACOS_TERMINALS = [
  { value: "terminal", labelKey: "settings.terminal.options.macos.terminal" },
  { value: "iterm2", labelKey: "settings.terminal.options.macos.iterm2" },
  { value: "alacritty", labelKey: "settings.terminal.options.macos.alacritty" },
  { value: "kitty", labelKey: "settings.terminal.options.macos.kitty" },
  { value: "ghostty", labelKey: "settings.terminal.options.macos.ghostty" },
  { value: "wezterm", labelKey: "settings.terminal.options.macos.wezterm" },
  { value: "kaku", labelKey: "settings.terminal.options.macos.kaku" },
  { value: "warp", labelKey: "settings.terminal.options.macos.warp" },
] as const;

const WINDOWS_TERMINALS = [
  { value: "cmd", labelKey: "settings.terminal.options.windows.cmd" },
  {
    value: "powershell",
    labelKey: "settings.terminal.options.windows.powershell",
  },
  { value: "wt", labelKey: "settings.terminal.options.windows.wt" },
] as const;

const LINUX_TERMINALS = [
  {
    value: "gnome-terminal",
    labelKey: "settings.terminal.options.linux.gnomeTerminal",
  },
  { value: "konsole", labelKey: "settings.terminal.options.linux.konsole" },
  {
    value: "xfce4-terminal",
    labelKey: "settings.terminal.options.linux.xfce4Terminal",
  },
  { value: "alacritty", labelKey: "settings.terminal.options.linux.alacritty" },
  { value: "kitty", labelKey: "settings.terminal.options.linux.kitty" },
  { value: "ghostty", labelKey: "settings.terminal.options.linux.ghostty" },
] as const;

// Get terminals for the current platform
function getTerminalOptions() {
  if (isMac()) {
    return MACOS_TERMINALS;
  }
  if (isWindows()) {
    return WINDOWS_TERMINALS;
  }
  if (isLinux()) {
    return LINUX_TERMINALS;
  }
  // Fallback to macOS options
  return MACOS_TERMINALS;
}

// Get default terminal for the current platform
function getDefaultTerminal(): string {
  if (isMac()) {
    return "terminal";
  }
  if (isWindows()) {
    return "cmd";
  }
  if (isLinux()) {
    return "gnome-terminal";
  }
  return "terminal";
}

export interface TerminalSettingsProps {
  value?: string;
  onChange: (value: string) => void;
  customTerminals?: CustomTerminal[];
  onCustomTerminalsChange?: (terminals: CustomTerminal[]) => void;
}

export function TerminalSettings({
  value,
  onChange,
  customTerminals = [],
  onCustomTerminalsChange,
}: TerminalSettingsProps) {
  const { t } = useTranslation();
  const terminals = getTerminalOptions();
  const defaultTerminal = getDefaultTerminal();

  // Use value or default
  const currentValue = value || defaultTerminal;

  // Handle browsing for a terminal application
  const handleBrowseTerminal = async () => {
    // Configure dialog based on platform
    const defaultPath = isMac()
      ? "/Applications"
      : isWindows()
        ? "C:\\Program Files"
        : "/usr/bin";
    const filters = isMac()
      ? [{ name: "Applications", extensions: ["app"] }]
      : isWindows()
        ? [{ name: "Executables", extensions: ["exe"] }]
        : undefined; // Linux: show all files

    const selected = await open({
      defaultPath,
      filters,
      directory: false,
      multiple: false,
      title: t("settings.terminal.browseTitle"),
    });

    if (selected && typeof selected === "string") {
      // Extract app name from path
      // macOS: /Applications/Warp.app -> Warp
      // Windows: C:\Program Files\App\app.exe -> app
      // Linux: /usr/bin/alacritty -> alacritty
      const fileName = selected.split(/[/\\]/).pop() || "";
      const label = fileName.replace(/\.(app|exe)$/i, "") || "Unknown";

      const newTerminal: CustomTerminal = {
        value: selected,
        label,
      };

      onCustomTerminalsChange?.([...customTerminals, newTerminal]);
    }
  };

  // Handle removing a custom terminal
  const handleRemoveCustomTerminal = (terminalValue: string) => {
    const updated = customTerminals.filter((t) => t.value !== terminalValue);
    onCustomTerminalsChange?.(updated);

    // If the removed terminal was selected, switch to default
    if (value === terminalValue) {
      onChange(defaultTerminal);
    }
  };

  return (
    <section className="space-y-4">
      <header className="space-y-1">
        <h3 className="text-sm font-medium">{t("settings.terminal.title")}</h3>
        <p className="text-xs text-muted-foreground">
          {t("settings.terminal.description")}
        </p>
      </header>

      {/* Terminal Selection */}
      <Select value={currentValue} onValueChange={onChange}>
        <SelectTrigger className="w-[200px]">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {/* Preset terminals */}
          {terminals.map((terminal) => (
            <SelectItem key={terminal.value} value={terminal.value}>
              {t(terminal.labelKey)}
            </SelectItem>
          ))}
          {/* Custom terminals */}
          {customTerminals.map((terminal) => (
            <SelectItem key={terminal.value} value={terminal.value}>
              {terminal.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>

      <p className="text-xs text-muted-foreground">
        {t("settings.terminal.fallbackHint")}
      </p>

      {/* Custom Terminals Section */}
      <div className="space-y-3 pt-2 border-t">
        <h4 className="text-sm font-medium">
          {t("settings.terminal.customTerminals")}
        </h4>

        {/* Add custom terminal via file picker */}
        <Button
          variant="outline"
          size="sm"
          onClick={handleBrowseTerminal}
          className="h-8 gap-2"
        >
          <FolderOpen className="h-4 w-4" />
          {t("settings.terminal.browseTerminal")}
        </Button>

        {/* List of custom terminals */}
        {customTerminals.length > 0 ? (
          <ul className="space-y-2">
            {customTerminals.map((terminal) => (
              <li
                key={terminal.value}
                className="flex items-center justify-between p-2 rounded-md bg-muted/50"
              >
                <div className="flex flex-col">
                  <span className="text-sm font-medium">{terminal.label}</span>
                  <span className="text-xs text-muted-foreground font-mono">
                    {terminal.value}
                  </span>
                </div>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => handleRemoveCustomTerminal(terminal.value)}
                  className="h-8 w-8 p-0 text-muted-foreground hover:text-destructive"
                >
                  <Trash2 className="h-4 w-4" />
                </Button>
              </li>
            ))}
          </ul>
        ) : (
          <p className="text-xs text-muted-foreground italic">
            {t("settings.terminal.noCustomTerminals")}
          </p>
        )}
      </div>
    </section>
  );
}
