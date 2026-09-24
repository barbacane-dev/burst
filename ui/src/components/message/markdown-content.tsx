import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { remarkMentions } from "../../lib/mentions";

export function MarkdownContent({ content }: { content: string }) {
  return (
    <ReactMarkdown
      remarkPlugins={[remarkGfm, remarkMentions]}
      components={{
        p: ({ children }) => <p className="text-sm text-gray-800 dark:text-gray-200">{children}</p>,
        strong: ({ children }) => <strong className="font-semibold">{children}</strong>,
        em: ({ children }) => <em>{children}</em>,
        del: ({ children }) => <del className="line-through">{children}</del>,
        a: ({ href, children }) => (
          <a
            href={href}
            target="_blank"
            rel="noopener noreferrer"
            className="text-indigo-600 underline hover:text-indigo-500 dark:text-indigo-400"
          >
            {children}
          </a>
        ),
        code: ({ className, children }) => {
          const isBlock = className?.startsWith("language-");
          if (isBlock) {
            return (
              <code className="block overflow-x-auto rounded-md bg-gray-100 p-3 text-xs font-mono text-gray-800 dark:bg-gray-800 dark:text-gray-200">
                {children}
              </code>
            );
          }
          return (
            <code className="rounded bg-gray-100 px-1 py-0.5 text-xs font-mono text-pink-600 dark:bg-gray-800 dark:text-pink-400">
              {children}
            </code>
          );
        },
        pre: ({ children }) => <pre className="my-1">{children}</pre>,
        ul: ({ children }) => <ul className="ml-4 list-disc text-sm text-gray-800 dark:text-gray-200">{children}</ul>,
        ol: ({ children }) => <ol className="ml-4 list-decimal text-sm text-gray-800 dark:text-gray-200">{children}</ol>,
        li: ({ children }) => <li className="mt-0.5">{children}</li>,
        blockquote: ({ children }) => (
          <blockquote className="border-l-2 border-gray-300 pl-3 text-sm italic text-gray-500 dark:border-gray-600 dark:text-gray-400">
            {children}
          </blockquote>
        ),
        hr: () => <hr className="my-2 border-gray-200 dark:border-gray-700" />,
        table: ({ children }) => (
          <div className="my-1 overflow-x-auto">
            <table className="min-w-full text-xs">{children}</table>
          </div>
        ),
        th: ({ children }) => (
          <th className="border border-gray-200 bg-gray-50 px-2 py-1 text-left font-semibold dark:border-gray-700 dark:bg-gray-800">
            {children}
          </th>
        ),
        td: ({ children }) => (
          <td className="border border-gray-200 px-2 py-1 dark:border-gray-700">{children}</td>
        ),
      }}
    >
      {content}
    </ReactMarkdown>
  );
}
