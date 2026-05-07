import { useState, useMemo } from "react";
import { useTranslation } from "react-i18next";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Badge } from "@/components/ui/badge";
import { Terminal, Globe, Radio } from "lucide-react";
import type { McpServer, McpServerSpec } from "@/types";

interface ParsedMcpEntry {
  name: string;
  server: McpServerSpec;
}

interface McpImportPreviewModalProps {
  isOpen: boolean;
  onClose: () => void;
  servers: ParsedMcpEntry[];
  onImport: (servers: McpServer[]) => void;
}

const TYPE_ICON: Record<string, React.ReactNode> = {
  stdio: <Terminal className="h-3 w-3" />,
  http: <Globe className="h-3 w-3" />,
  sse: <Radio className="h-3 w-3" />,
};

export function McpImportPreviewModal({
  isOpen,
  onClose,
  servers,
  onImport,
}: McpImportPreviewModalProps) {
  const { t } = useTranslation();
  const [selectedNames, setSelectedNames] = useState<Set<string>>(new Set());

  useMemo(() => {
    setSelectedNames(new Set(servers.map((s) => s.name)));
  }, [servers]);

  const allSelected = selectedNames.size === servers.length;

  const toggleAll = () => {
    if (allSelected) {
      setSelectedNames(new Set());
    } else {
      setSelectedNames(new Set(servers.map((s) => s.name)));
    }
  };

  const toggleOne = (name: string) => {
    setSelectedNames((prev) => {
      const next = new Set(prev);
      if (next.has(name)) {
        next.delete(name);
      } else {
        next.add(name);
      }
      return next;
    });
  };

  const handleImport = () => {
    const selected = servers
      .filter((s) => selectedNames.has(s.name))
      .map((s) => ({
        id: crypto.randomUUID(),
        name: s.name,
        server: s.server,
        apps: {
          claude: false,
          codex: false,
          gemini: false,
          opencode: false,
          openclaw: false,
          hermes: false,
        },
        tags: [],
      }));
    onImport(selected);
  };

  const getServerSummary = (server: McpServerSpec): string => {
    if (server.type === "stdio") {
      return server.command || "";
    }
    return server.url || "";
  };

  return (
    <Dialog open={isOpen} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>{t("mcp.importJson.title")}</DialogTitle>
        </DialogHeader>

        <div className="space-y-3">
          <div className="flex items-center justify-between">
            <Button variant="ghost" size="sm" onClick={toggleAll}>
              {allSelected
                ? t("mcp.importJson.deselectAll")
                : t("mcp.importJson.selectAll")}
            </Button>
            <span className="text-sm text-muted-foreground">
              {t("mcp.importJson.selected", { count: selectedNames.size })}
            </span>
          </div>

          <div className="max-h-[400px] overflow-y-auto rounded-md border">
            {servers.map((entry) => (
              <label
                key={entry.name}
                className="flex cursor-pointer items-center gap-3 border-b px-4 py-3 last:border-b-0 hover:bg-muted/50"
              >
                <Checkbox
                  checked={selectedNames.has(entry.name)}
                  onCheckedChange={() => toggleOne(entry.name)}
                />
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2">
                    <span className="font-medium truncate">{entry.name}</span>
                    <Badge
                      variant="outline"
                      className="flex items-center gap-1 text-xs"
                    >
                      {TYPE_ICON[entry.server.type || "stdio"]}
                      {entry.server.type || "stdio"}
                    </Badge>
                  </div>
                  <p className="text-xs text-muted-foreground truncate mt-0.5">
                    {getServerSummary(entry.server)}
                  </p>
                </div>
              </label>
            ))}
          </div>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={onClose}>
            {t("common.cancel")}
          </Button>
          <Button onClick={handleImport} disabled={selectedNames.size === 0}>
            {t("mcp.importJson.import")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
