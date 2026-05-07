import React, { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Server, Search, Plug, Loader2 } from "lucide-react";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { TooltipProvider } from "@/components/ui/tooltip";
import {
  useAllMcpServers,
  useToggleMcpApp,
  useImportMcpFromApps,
  useTestMcpConnectivity,
} from "@/hooks/useMcp";
import type { McpServer, McpServerSpec } from "@/types";
import type { AppId } from "@/lib/api/types";
import McpFormModal from "./McpFormModal";
import { ConfirmDialog } from "../ConfirmDialog";
import { Edit3, Trash2, ExternalLink } from "lucide-react";
import { settingsApi } from "@/lib/api";
import { mcpPresets } from "@/config/mcpPresets";
import { toast } from "sonner";
import { MCP_APP_IDS } from "@/config/appConfig";
import { AppCountBar } from "@/components/common/AppCountBar";
import { AppToggleGroup } from "@/components/common/AppToggleGroup";
import { ListItemRow } from "@/components/common/ListItemRow";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { mcpApi } from "@/lib/api/mcp";
import { McpImportPreviewModal } from "./McpImportPreviewModal";

interface UnifiedMcpPanelProps {
  onOpenChange: (open: boolean) => void;
}

export interface UnifiedMcpPanelHandle {
  openAdd: () => void;
  openImport: () => void;
  openJsonImport: () => void;
}

const UnifiedMcpPanel = React.forwardRef<
  UnifiedMcpPanelHandle,
  UnifiedMcpPanelProps
>(({ onOpenChange: _onOpenChange }, ref) => {
  const { t } = useTranslation();
  const [isFormOpen, setIsFormOpen] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [filterApp, setFilterApp] = useState<AppId | "all">("all");
  const [filterStatus, setFilterStatus] = useState<
    "all" | "enabled" | "disabled"
  >("all");
  const [confirmDialog, setConfirmDialog] = useState<{
    isOpen: boolean;
    title: string;
    message: string;
    confirmText?: string;
    onConfirm: () => void;
  } | null>(null);
  const [selectedServerIds, setSelectedServerIds] = useState<Set<string>>(
    () => new Set(),
  );
  const [isBatchDeleting, setIsBatchDeleting] = useState(false);

  const { data: serversMap, isLoading } = useAllMcpServers();
  const toggleAppMutation = useToggleMcpApp();
  const importMutation = useImportMcpFromApps();
  const [isImportPreviewOpen, setIsImportPreviewOpen] = useState(false);
  const [importedServers, setImportedServers] = useState<
    { name: string; server: McpServerSpec }[]
  >([]);

  const serverEntries = useMemo((): Array<[string, McpServer]> => {
    if (!serversMap) return [];
    return Object.entries(serversMap);
  }, [serversMap]);

  const enabledCounts = useMemo(() => {
    const counts = {
      claude: 0,
      codex: 0,
      gemini: 0,
      opencode: 0,
      openclaw: 0,
      hermes: 0,
    };
    serverEntries.forEach(([_, server]) => {
      for (const app of MCP_APP_IDS) {
        if (server.apps[app]) counts[app]++;
      }
    });
    return counts;
  }, [serverEntries]);

  const filteredServers = useMemo(() => {
    return serverEntries.filter(([id, server]) => {
      if (searchQuery.trim()) {
        const query = searchQuery.toLowerCase();
        const name = server.name || id;
        const description = server.description || "";
        const matchesSearch =
          name.toLowerCase().includes(query) ||
          description.toLowerCase().includes(query);
        if (!matchesSearch) return false;
      }

      if (filterApp !== "all" && !server.apps[filterApp]) {
        return false;
      }

      const isEnabled = Object.values(server.apps).some(Boolean);
      if (filterStatus === "enabled" && !isEnabled) return false;
      if (filterStatus === "disabled" && isEnabled) return false;

      return true;
    });
  }, [serverEntries, searchQuery, filterApp, filterStatus]);

  const selectedServers = useMemo(
    () => filteredServers.filter(([id]) => selectedServerIds.has(id)),
    [filteredServers, selectedServerIds],
  );

  const allFilteredSelected =
    filteredServers.length > 0 &&
    filteredServers.every(([id]) => selectedServerIds.has(id));

  const handleToggleApp = async (
    serverId: string,
    app: AppId,
    enabled: boolean,
  ) => {
    try {
      await toggleAppMutation.mutateAsync({ serverId, app, enabled });
    } catch (error) {
      toast.error(t("common.error"), { description: String(error) });
    }
  };

  const handleEdit = (id: string) => {
    setEditingId(id);
    setIsFormOpen(true);
  };

  const handleAdd = () => {
    setEditingId(null);
    setIsFormOpen(true);
  };

  const handleImport = async () => {
    try {
      const count = await importMutation.mutateAsync();
      if (count === 0) {
        toast.success(t("mcp.unifiedPanel.noImportFound"), {
          closeButton: true,
        });
      } else {
        toast.success(t("mcp.unifiedPanel.importSuccess", { count }), {
          closeButton: true,
        });
      }
    } catch (error) {
      toast.error(t("common.error"), { description: String(error) });
    }
  };

  const handleJsonImport = async () => {
    try {
      const selected = await openFileDialog({
        multiple: false,
        filters: [{ name: "JSON", extensions: ["json", "jsonc"] }],
      });
      if (!selected) return;
      const filePath = selected;
      const servers = await mcpApi.parseMcpJsonFile(filePath);
      if (servers.length === 0) {
        toast.info(t("mcp.importJson.empty"));
        return;
      }
      setImportedServers(servers);
      setIsImportPreviewOpen(true);
    } catch {
      toast.error(t("mcp.importJson.parseError"));
    }
  };

  const handleImportConfirm = async (servers: McpServer[]) => {
    let count = 0;
    for (const server of servers) {
      try {
        await mcpApi.upsertUnifiedServer(server);
        count++;
      } catch {
        // skip failed entries
      }
    }
    if (count > 0) {
      toast.success(t("mcp.importJson.success", { count }));
    }
    setIsImportPreviewOpen(false);
    setImportedServers([]);
  };

  React.useImperativeHandle(ref, () => ({
    openAdd: handleAdd,
    openImport: handleImport,
    openJsonImport: handleJsonImport,
  }));

  const handleDelete = (id: string) => {
    setConfirmDialog({
      isOpen: true,
      title: t("mcp.unifiedPanel.deleteServer"),
      message: t("mcp.unifiedPanel.deleteConfirm", { id }),
      onConfirm: async () => {
        try {
          await mcpApi.deleteUnifiedServer(id);
          setSelectedServerIds((current) => {
            const next = new Set(current);
            next.delete(id);
            return next;
          });
          setConfirmDialog(null);
          toast.success(t("common.success"), { closeButton: true });
        } catch (error) {
          toast.error(t("common.error"), { description: String(error) });
        }
      },
    });
  };

  const handleToggleSelect = (id: string, checked: boolean) => {
    setSelectedServerIds((current) => {
      const next = new Set(current);
      if (checked) next.add(id);
      else next.delete(id);
      return next;
    });
  };

  const handleToggleSelectAll = () => {
    setSelectedServerIds((current) => {
      const next = new Set(current);
      if (allFilteredSelected) {
        filteredServers.forEach(([id]) => next.delete(id));
      } else {
        filteredServers.forEach(([id]) => next.add(id));
      }
      return next;
    });
  };

  const handleBatchDelete = () => {
    if (selectedServers.length === 0 || isBatchDeleting) return;
    setConfirmDialog({
      isOpen: true,
      title: t("mcp.unifiedPanel.bulkDelete.title"),
      message: t("mcp.unifiedPanel.bulkDelete.message", {
        count: selectedServers.length,
      }),
      confirmText: t("mcp.unifiedPanel.bulkDelete.confirm", {
        count: selectedServers.length,
      }),
      onConfirm: async () => {
        const loadingToast = toast.loading(
          t("mcp.unifiedPanel.bulkDelete.progress", {
            count: selectedServers.length,
          }),
        );
        setConfirmDialog(null);
        setIsBatchDeleting(true);
        let successCount = 0;
        let firstError: string | null = null;
        try {
          for (const [id] of selectedServers) {
            try {
              await mcpApi.deleteUnifiedServer(id);
              successCount++;
            } catch (error) {
              firstError ??= String(error);
            }
          }
          setSelectedServerIds(new Set());
          if (successCount > 0) {
            toast.success(
              t("mcp.unifiedPanel.bulkDelete.success", { count: successCount }),
              { id: loadingToast, closeButton: true },
            );
          } else {
            toast.error(t("mcp.unifiedPanel.bulkDelete.failed"), {
              id: loadingToast,
              description: firstError ?? t("common.unknown"),
            });
          }
          if (firstError && successCount > 0) {
            toast.error(
              t("mcp.unifiedPanel.bulkDelete.partialFailed", {
                failed: selectedServers.length - successCount,
              }),
              { description: firstError },
            );
          }
        } finally {
          setIsBatchDeleting(false);
        }
      },
    });
  };

  const handleCloseForm = () => {
    setIsFormOpen(false);
    setEditingId(null);
  };

  return (
    <div className="px-6 flex flex-col flex-1 min-h-0 overflow-hidden">
      <AppCountBar
        totalLabel={t("mcp.serverCount", { count: serverEntries.length })}
        counts={enabledCounts}
        appIds={MCP_APP_IDS}
      />

      <div className="flex gap-3 mb-3">
        <div className="relative flex-1">
          <Search
            className="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground"
            size={16}
          />
          <Input
            placeholder={t("mcp.unifiedPanel.filter.searchPlaceholder")}
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="pl-9"
          />
        </div>
        <Select
          value={filterApp}
          onValueChange={(v) => setFilterApp(v as AppId | "all")}
        >
          <SelectTrigger className="w-[130px]">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">
              {t("mcp.unifiedPanel.filter.allApps")}
            </SelectItem>
            <SelectItem value="claude">Claude</SelectItem>
            <SelectItem value="codex">Codex</SelectItem>
            <SelectItem value="gemini">Gemini</SelectItem>
            <SelectItem value="opencode">OpenCode</SelectItem>
          </SelectContent>
        </Select>
        <Select
          value={filterStatus}
          onValueChange={(v) =>
            setFilterStatus(v as "all" | "enabled" | "disabled")
          }
        >
          <SelectTrigger className="w-[130px]">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">
              {t("mcp.unifiedPanel.filter.allStatus")}
            </SelectItem>
            <SelectItem value="enabled">
              {t("mcp.unifiedPanel.filter.enabled")}
            </SelectItem>
            <SelectItem value="disabled">
              {t("mcp.unifiedPanel.filter.disabled")}
            </SelectItem>
          </SelectContent>
        </Select>
        <Button
          type="button"
          variant="outline"
          size="sm"
          className="shrink-0"
          onClick={handleBatchDelete}
          disabled={selectedServers.length === 0 || isBatchDeleting}
        >
          {isBatchDeleting ? (
            <Loader2 size={16} className="mr-1.5 animate-spin" />
          ) : (
            <Trash2 size={16} className="mr-1.5" />
          )}
          {isBatchDeleting
            ? t("mcp.unifiedPanel.bulkDelete.deleting")
            : t("mcp.unifiedPanel.bulkDelete.button", {
                count: selectedServers.length,
              })}
        </Button>
      </div>

      {(filteredServers.length > 0 || selectedServers.length > 0) && (
        <div className="flex items-center justify-between gap-3 mb-4 rounded-lg border border-border-default bg-muted/40 px-3 py-2 text-sm text-muted-foreground">
          <div className="flex items-center gap-2">
            <Badge variant="secondary">
              {t("common.selectedCount", { count: selectedServers.length })}
            </Badge>
            <span>{t("common.batchModeHint")}</span>
          </div>
          <div className="flex items-center gap-2">
            {filteredServers.length > 0 && (
              <Button
                variant="ghost"
                size="sm"
                className="h-7 px-2.5 text-xs"
                onClick={handleToggleSelectAll}
              >
                {allFilteredSelected
                  ? t("common.clearFilteredSelection")
                  : t("common.selectAllFiltered")}
              </Button>
            )}
            <Button
              variant="ghost"
              size="sm"
              className="h-7 px-2.5 text-xs"
              onClick={() => setSelectedServerIds(new Set())}
              disabled={selectedServers.length === 0}
            >
              {t("common.clearSelection")}
            </Button>
          </div>
        </div>
      )}

      <div className="flex-1 overflow-y-auto overflow-x-hidden pb-24">
        {isLoading ? (
          <div className="text-center py-12 text-muted-foreground">
            {t("mcp.loading")}
          </div>
        ) : serverEntries.length === 0 ? (
          <div className="text-center py-12">
            <div className="w-16 h-16 mx-auto mb-4 bg-muted rounded-full flex items-center justify-center">
              <Server size={24} className="text-muted-foreground" />
            </div>
            <h3 className="text-lg font-medium text-foreground mb-2">
              {t("mcp.unifiedPanel.noServers")}
            </h3>
            <p className="text-muted-foreground text-sm">
              {t("mcp.emptyDescription")}
            </p>
          </div>
        ) : filteredServers.length === 0 ? (
          <div className="text-center py-12">
            <div className="w-16 h-16 mx-auto mb-4 bg-muted rounded-full flex items-center justify-center">
              <Search size={24} className="text-muted-foreground" />
            </div>
            <h3 className="text-lg font-medium text-foreground mb-2">
              {t("mcp.unifiedPanel.filter.noResults")}
            </h3>
            <p className="text-muted-foreground text-sm">
              {t("mcp.unifiedPanel.filter.noResultsHint")}
            </p>
          </div>
        ) : (
          <TooltipProvider delayDuration={300}>
            <div className="rounded-xl border border-border-default overflow-hidden">
              {filteredServers.map(([id, server], index) => (
                <UnifiedMcpListItem
                  key={id}
                  id={id}
                  server={server}
                  onToggleApp={handleToggleApp}
                  onEdit={handleEdit}
                  onDelete={handleDelete}
                  selectionMode
                  isChecked={selectedServerIds.has(id)}
                  onToggleChecked={(checked) => handleToggleSelect(id, checked)}
                  isLast={index === filteredServers.length - 1}
                />
              ))}
            </div>
          </TooltipProvider>
        )}
      </div>

      {isFormOpen && (
        <McpFormModal
          editingId={editingId || undefined}
          initialData={
            editingId && serversMap ? serversMap[editingId] : undefined
          }
          existingIds={serversMap ? Object.keys(serversMap) : []}
          defaultFormat="json"
          onSave={async () => {
            setIsFormOpen(false);
            setEditingId(null);
          }}
          onClose={handleCloseForm}
        />
      )}

      {confirmDialog && (
        <ConfirmDialog
          isOpen={confirmDialog.isOpen}
          title={confirmDialog.title}
          message={confirmDialog.message}
          confirmText={confirmDialog.confirmText}
          onConfirm={confirmDialog.onConfirm}
          onCancel={() => setConfirmDialog(null)}
        />
      )}

      <McpImportPreviewModal
        isOpen={isImportPreviewOpen}
        onClose={() => setIsImportPreviewOpen(false)}
        servers={importedServers}
        onImport={handleImportConfirm}
      />
    </div>
  );
});

UnifiedMcpPanel.displayName = "UnifiedMcpPanel";

interface UnifiedMcpListItemProps {
  id: string;
  server: McpServer;
  onToggleApp: (serverId: string, app: AppId, enabled: boolean) => void;
  onEdit: (id: string) => void;
  onDelete: (id: string) => void;
  selectionMode?: boolean;
  isChecked?: boolean;
  onToggleChecked?: (checked: boolean) => void;
  isLast?: boolean;
}

interface TestResult {
  ok: boolean;
  message: string;
  serverName?: string;
  serverVersion?: string;
}

const UnifiedMcpListItem: React.FC<UnifiedMcpListItemProps> = ({
  id,
  server,
  onToggleApp,
  onEdit,
  onDelete,
  selectionMode = false,
  isChecked = false,
  onToggleChecked,
  isLast,
}) => {
  const { t } = useTranslation();
  const testMutation = useTestMcpConnectivity();
  const [testResult, setTestResult] = useState<TestResult | null>(null);

  const handleTest = async () => {
    try {
      const result = await testMutation.mutateAsync(server.server);
      setTestResult({
        ok: result.ok,
        message: result.message,
        serverName: result.server_name,
        serverVersion: result.server_version,
      });
      if (result.ok) {
        const detail = result.server_name
          ? `${result.server_name}${result.server_version ? ` v${result.server_version}` : ""}`
          : result.message;
        toast.success(t("mcp.connectivity.success", { message: detail }));
      } else {
        toast.error(t("mcp.connectivity.failed", { message: result.message }));
      }
    } catch (err) {
      const msg = String(err);
      setTestResult({ ok: false, message: msg });
      toast.error(t("mcp.connectivity.failed", { message: msg }));
    }
  };

  const name = server.name || id;
  const description = server.description || "";

  const meta = mcpPresets.find((p) => p.id === id);
  const docsUrl = server.docs || meta?.docs;
  const homepageUrl = server.homepage || meta?.homepage;
  const tags = server.tags || meta?.tags;

  const openDocs = async () => {
    const url = docsUrl || homepageUrl;
    if (!url) return;
    try {
      await settingsApi.openExternal(url);
    } catch {
      // ignore
    }
  };

  const statusDotTitle = testResult
    ? testResult.ok
      ? testResult.serverName
        ? `${testResult.serverName}${testResult.serverVersion ? ` v${testResult.serverVersion}` : ""}`
        : testResult.message
      : testResult.message
    : undefined;

  return (
    <ListItemRow isLast={isLast}>
      {selectionMode && (
        <Checkbox
          checked={isChecked}
          aria-label={t("common.select")}
          onCheckedChange={(checked) => onToggleChecked?.(Boolean(checked))}
        />
      )}

      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-1.5">
          <span className="font-medium text-sm text-foreground truncate">
            {name}
          </span>
          {testResult !== null && (
            <span
              className={`inline-block w-2 h-2 rounded-full flex-shrink-0 ${
                testResult.ok ? "bg-green-500" : "bg-red-500"
              }`}
              title={statusDotTitle}
            />
          )}
          {docsUrl && (
            <button
              type="button"
              onClick={openDocs}
              className="text-muted-foreground/60 hover:text-foreground flex-shrink-0"
              title={t("mcp.presets.docs")}
            >
              <ExternalLink size={12} />
            </button>
          )}
        </div>
        {description && (
          <p
            className="text-xs text-muted-foreground truncate"
            title={description}
          >
            {description}
          </p>
        )}
        {!description && tags && tags.length > 0 && (
          <p className="text-xs text-muted-foreground/60 truncate">
            {tags.join(", ")}
          </p>
        )}
      </div>

      <AppToggleGroup
        apps={server.apps}
        onToggle={(app, enabled) => onToggleApp(id, app, enabled)}
        appIds={MCP_APP_IDS}
      />

      <div className="flex items-center gap-0.5 flex-shrink-0 opacity-40 group-hover:opacity-100 transition-opacity">
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="h-7 w-7"
          onClick={handleTest}
          disabled={testMutation.isPending}
          title={t("mcp.connectivity.test")}
        >
          {testMutation.isPending ? (
            <Loader2 size={14} className="animate-spin" />
          ) : (
            <Plug size={14} />
          )}
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="h-7 w-7"
          onClick={() => onEdit(id)}
          title={t("common.edit")}
        >
          <Edit3 size={14} />
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="h-7 w-7 hover:text-red-500 hover:bg-red-100 dark:hover:text-red-400 dark:hover:bg-red-500/10"
          onClick={() => onDelete(id)}
          title={t("common.delete")}
        >
          <Trash2 size={14} />
        </Button>
      </div>
    </ListItemRow>
  );
};

export default UnifiedMcpPanel;
