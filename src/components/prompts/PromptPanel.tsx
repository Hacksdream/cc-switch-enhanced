import React, { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { FileText, Search, Trash2, Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { type AppId, promptsApi } from "@/lib/api";
import { usePromptActions } from "@/hooks/usePromptActions";
import PromptListItem from "./PromptListItem";
import PromptFormPanel from "./PromptFormPanel";
import { ConfirmDialog } from "../ConfirmDialog";
import { Input } from "@/components/ui/input";
import { toast } from "sonner";

interface PromptPanelProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  appId: AppId;
}

export interface PromptPanelHandle {
  openAdd: () => void;
}

const PromptPanel = React.forwardRef<PromptPanelHandle, PromptPanelProps>(
  ({ open, appId }, ref) => {
    const { t } = useTranslation();
    const [isFormOpen, setIsFormOpen] = useState(false);
    const [editingId, setEditingId] = useState<string | null>(null);
    const [confirmDialog, setConfirmDialog] = useState<{
      isOpen: boolean;
      titleKey: string;
      messageKey: string;
      messageParams?: Record<string, unknown>;
      confirmText?: string;
      onConfirm: () => void;
    } | null>(null);
    const [searchQuery, setSearchQuery] = useState("");
    const [selectedPromptIds, setSelectedPromptIds] = useState<Set<string>>(
      () => new Set(),
    );
    const [isBatchDeleting, setIsBatchDeleting] = useState(false);

    const {
      prompts,
      loading,
      reload,
      savePrompt,
      deletePrompt,
      toggleEnabled,
    } = usePromptActions(appId);

    useEffect(() => {
      if (open) reload();
    }, [open, reload]);

    useEffect(() => {
      const handlePromptImported = (event: Event) => {
        const customEvent = event as CustomEvent;
        if (customEvent.detail?.app === appId) {
          reload();
        }
      };

      window.addEventListener("prompt-imported", handlePromptImported);
      return () => {
        window.removeEventListener("prompt-imported", handlePromptImported);
      };
    }, [appId, reload]);

    const handleAdd = () => {
      setEditingId(null);
      setIsFormOpen(true);
    };

    React.useImperativeHandle(ref, () => ({
      openAdd: handleAdd,
    }));

    const handleEdit = (id: string) => {
      setEditingId(id);
      setIsFormOpen(true);
    };

    const promptEntries = useMemo(() => Object.entries(prompts), [prompts]);

    const filteredPrompts = useMemo(() => {
      if (!searchQuery.trim()) return promptEntries;
      const query = searchQuery.toLowerCase();
      return promptEntries.filter(([_, prompt]) => {
        const nameMatch = prompt.name?.toLowerCase().includes(query);
        const contentMatch = prompt.content?.toLowerCase().includes(query);
        return nameMatch || contentMatch;
      });
    }, [promptEntries, searchQuery]);

    const selectedPrompts = useMemo(
      () => filteredPrompts.filter(([id]) => selectedPromptIds.has(id)),
      [filteredPrompts, selectedPromptIds],
    );

    const allFilteredSelected =
      filteredPrompts.length > 0 &&
      filteredPrompts.every(([id]) => selectedPromptIds.has(id));

    const enabledPrompt = promptEntries.find(([_, p]) => p.enabled);

    const handleDelete = (id: string) => {
      const prompt = prompts[id];
      setConfirmDialog({
        isOpen: true,
        titleKey: "prompts.confirm.deleteTitle",
        messageKey: "prompts.confirm.deleteMessage",
        messageParams: { name: prompt?.name },
        onConfirm: async () => {
          try {
            await deletePrompt(id);
            setSelectedPromptIds((current) => {
              const next = new Set(current);
              next.delete(id);
              return next;
            });
            setConfirmDialog(null);
          } catch {
            // Error handled by hook
          }
        },
      });
    };

    const handleToggleSelect = (id: string, checked: boolean) => {
      setSelectedPromptIds((current) => {
        const next = new Set(current);
        if (checked) next.add(id);
        else next.delete(id);
        return next;
      });
    };

    const handleToggleSelectAll = () => {
      setSelectedPromptIds((current) => {
        const next = new Set(current);
        if (allFilteredSelected) {
          filteredPrompts.forEach(([id]) => next.delete(id));
        } else {
          filteredPrompts.forEach(([id]) => next.add(id));
        }
        return next;
      });
    };

    const handleBatchDelete = () => {
      if (selectedPrompts.length === 0 || isBatchDeleting) return;
      setConfirmDialog({
        isOpen: true,
        titleKey: "prompts.confirm.batchDeleteTitle",
        messageKey: "prompts.confirm.batchDeleteMessage",
        messageParams: { count: selectedPrompts.length },
        confirmText: t("prompts.bulkDelete.confirm", {
          count: selectedPrompts.length,
        }),
        onConfirm: async () => {
          const loadingToast = toast.loading(
            t("prompts.bulkDelete.progress", { count: selectedPrompts.length }),
          );
          setConfirmDialog(null);
          setIsBatchDeleting(true);
          let successCount = 0;
          let firstError: string | null = null;
          try {
            for (const [id] of selectedPrompts) {
              try {
                await promptsApi.deletePrompt(appId, id);
                successCount++;
              } catch (error) {
                firstError ??= String(error);
              }
            }
            await reload();
            setSelectedPromptIds(new Set());
            if (successCount > 0) {
              toast.success(
                t("prompts.bulkDelete.success", { count: successCount }),
                { id: loadingToast, closeButton: true },
              );
            } else {
              toast.error(t("prompts.bulkDelete.failed"), {
                id: loadingToast,
                description: firstError ?? t("common.unknown"),
              });
            }
            if (firstError && successCount > 0) {
              toast.error(
                t("prompts.bulkDelete.partialFailed", {
                  failed: selectedPrompts.length - successCount,
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

    return (
      <div className="flex flex-col flex-1 min-h-0 px-6">
        <div className="flex-shrink-0 py-4 glass rounded-xl border border-white/10 mb-4 px-6">
          <div className="text-sm text-muted-foreground">
            {t("prompts.count", { count: promptEntries.length })} ·{" "}
            {enabledPrompt
              ? t("prompts.enabledName", { name: enabledPrompt[1].name })
              : t("prompts.noneEnabled")}
          </div>
        </div>

        <div className="flex items-center gap-3 mb-3">
          <div className="relative flex-1">
            <Search
              className="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground"
              size={16}
            />
            <Input
              placeholder={t("prompts.filter.search")}
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="pl-9"
            />
          </div>
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="shrink-0"
            onClick={handleBatchDelete}
            disabled={selectedPrompts.length === 0 || isBatchDeleting}
          >
            {isBatchDeleting ? (
              <Loader2 size={16} className="mr-1.5 animate-spin" />
            ) : (
              <Trash2 size={16} className="mr-1.5" />
            )}
            {isBatchDeleting
              ? t("prompts.bulkDelete.deleting")
              : t("prompts.bulkDelete.button", {
                  count: selectedPrompts.length,
                })}
          </Button>
        </div>

        {(filteredPrompts.length > 0 || selectedPrompts.length > 0) && (
          <div className="flex items-center justify-between gap-3 mb-4 rounded-lg border border-border-default bg-muted/40 px-3 py-2 text-sm text-muted-foreground">
            <div className="flex items-center gap-2">
              <Badge variant="secondary">
                {t("common.selectedCount", { count: selectedPrompts.length })}
              </Badge>
              <span>{t("common.batchModeHint")}</span>
            </div>
            <div className="flex items-center gap-2">
              {filteredPrompts.length > 0 && (
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
                onClick={() => setSelectedPromptIds(new Set())}
                disabled={selectedPrompts.length === 0}
              >
                {t("common.clearSelection")}
              </Button>
            </div>
          </div>
        )}

        <div className="flex-1 overflow-y-auto pb-16">
          {loading ? (
            <div className="text-center py-12 text-muted-foreground">
              {t("prompts.loading")}
            </div>
          ) : promptEntries.length === 0 ? (
            <div className="text-center py-12">
              <div className="w-16 h-16 mx-auto mb-4 bg-muted rounded-full flex items-center justify-center">
                <FileText size={24} className="text-muted-foreground" />
              </div>
              <h3 className="text-lg font-medium text-foreground mb-2">
                {t("prompts.empty")}
              </h3>
              <p className="text-muted-foreground text-sm">
                {t("prompts.emptyDescription")}
              </p>
            </div>
          ) : filteredPrompts.length === 0 ? (
            <div className="text-center py-12">
              <div className="w-16 h-16 mx-auto mb-4 bg-muted rounded-full flex items-center justify-center">
                <Search size={24} className="text-muted-foreground" />
              </div>
              <h3 className="text-lg font-medium text-foreground mb-2">
                {t("prompts.noFilterResults")}
              </h3>
              <p className="text-muted-foreground text-sm">
                {t("prompts.noFilterResultsDescription")}
              </p>
            </div>
          ) : (
            <div className="space-y-3">
              {filteredPrompts.map(([id, prompt]) => (
                <PromptListItem
                  key={id}
                  id={id}
                  prompt={prompt}
                  onToggle={toggleEnabled}
                  onEdit={handleEdit}
                  onDelete={handleDelete}
                  selectionMode
                  isChecked={selectedPromptIds.has(id)}
                  onToggleChecked={(checked) => handleToggleSelect(id, checked)}
                />
              ))}
            </div>
          )}
        </div>

        {isFormOpen && (
          <PromptFormPanel
            appId={appId}
            editingId={editingId || undefined}
            initialData={editingId ? prompts[editingId] : undefined}
            onSave={savePrompt}
            onClose={() => setIsFormOpen(false)}
          />
        )}

        {confirmDialog && (
          <ConfirmDialog
            isOpen={confirmDialog.isOpen}
            title={t(confirmDialog.titleKey)}
            message={t(confirmDialog.messageKey, confirmDialog.messageParams)}
            confirmText={confirmDialog.confirmText}
            onConfirm={confirmDialog.onConfirm}
            onCancel={() => setConfirmDialog(null)}
          />
        )}
      </div>
    );
  },
);

PromptPanel.displayName = "PromptPanel";

export default PromptPanel;
