import { FileText, Download } from "lucide-react";
import type { Attachment } from "../lib/api/types";
import { getAccessToken } from "../lib/api/client";

function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function downloadUrl(attachmentId: string): string {
  return `/api/attachments/${attachmentId}`;
}

export function AttachmentPreview({ attachment }: { attachment: Attachment }) {
  const isImage = attachment.contentType.startsWith("image/");
  const width = attachment.metadata.width as number | undefined;
  const height = attachment.metadata.height as number | undefined;

  const token = getAccessToken();
  const authParam = token ? `?access_token=${encodeURIComponent(token)}` : "";

  if (isImage) {
    return (
      <a
        href={downloadUrl(attachment.id) + authParam}
        target="_blank"
        rel="noopener noreferrer"
        className="mt-1 block max-w-xs"
      >
        <img
          src={downloadUrl(attachment.id) + authParam}
          alt={attachment.fileName}
          width={width}
          height={height}
          className="rounded-md border border-gray-200 dark:border-gray-700"
          style={{
            maxWidth: "100%",
            maxHeight: 300,
            ...(width && height ? { aspectRatio: `${width}/${height}` } : {}),
          }}
          loading="lazy"
        />
        <span className="mt-0.5 block text-xs text-gray-400">
          {attachment.fileName} ({formatFileSize(attachment.fileSize)})
        </span>
      </a>
    );
  }

  return (
    <a
      href={downloadUrl(attachment.id) + authParam}
      download={attachment.fileName}
      className="mt-1 flex items-center gap-2 rounded-md border border-gray-200 bg-gray-50 px-3 py-2 text-sm hover:bg-gray-100 dark:border-gray-700 dark:bg-gray-800 dark:hover:bg-gray-700"
      style={{ maxWidth: 280 }}
    >
      <FileText className="h-5 w-5 shrink-0 text-gray-400" />
      <div className="min-w-0 flex-1">
        <p className="truncate font-medium text-gray-700 dark:text-gray-300">
          {attachment.fileName}
        </p>
        <p className="text-xs text-gray-400">{formatFileSize(attachment.fileSize)}</p>
      </div>
      <Download className="h-4 w-4 shrink-0 text-gray-400" />
    </a>
  );
}
