<script lang="ts">
  import { LayoutGrid, Clock3, ChartNoAxesColumnIncreasing, ClipboardList, Settings, RefreshCw, Archive, Sparkles, Plus, Check, TriangleAlert, ChevronLeft, ChevronRight, ChevronDown, FolderOpen, KeyRound, ShieldCheck, Trash2, Download, X } from 'lucide-svelte';
  const icons = {overview:LayoutGrid,timeline:Clock3,analyze:ChartNoAxesColumnIncreasing,report:ClipboardList,settings:Settings,sync:RefreshCw,archive:Archive,sparkles:Sparkles,plus:Plus,check:Check,alert:TriangleAlert,clock:Clock3,'chevron-left':ChevronLeft,'chevron-right':ChevronRight,'chevron-down':ChevronDown,folder:FolderOpen,key:KeyRound,shield:ShieldCheck,trash:Trash2,download:Download,cancel:X};
  export let name:
    | 'overview'
    | 'timeline'
    | 'analyze'
    | 'report'
    | 'settings'
    | 'sync'
    | 'archive'
    | 'sparkles'
    | 'plus'
    | 'check'
    | 'alert'
    | 'clock'
    | 'chevron-left'
    | 'chevron-right'
    | 'chevron-down'
    | 'folder'
    | 'key'
    | 'shield'
    | 'trash'
    | 'download'
    | 'cancel';

  export let sizeClass = 'h-4 w-4';

</script>
<svelte:component this={icons[name]} class="{sizeClass} shrink-0" strokeWidth={1.75} aria-hidden="true" />
