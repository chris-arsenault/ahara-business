import type { ApiClient } from "./api";
import type { AttachmentUploadApi } from "./attachmentFiles";
import type {
  Contact,
  MailboxMessageDetail,
  MailboxMessageSummary,
  MailboxThreadDetail,
  OutboundMessageDetail,
  OutboundMessageSummary,
  ReplyMessageRequest,
} from "./types";

export type MailboxApi = Pick<ApiClient, "fetchMailboxMessages"> &
  Partial<
    Pick<
      ApiClient,
      | "fetchMessageDetail"
      | "fetchThreadDetail"
      | "downloadAttachment"
      | "downloadOutboundAttachment"
      | "createAttachmentUpload"
      | "updateMessageState"
      | "linkMessageContact"
      | "listContacts"
      | "searchMessages"
      | "composeMessage"
      | "replyToMessage"
      | "listOutboundMessages"
      | "fetchOutboundMessage"
    >
  >;

export type MailboxViewProps = {
  apiClient: MailboxApi;
};

export type MailboxState =
  | { status: "loading" }
  | { status: "ready"; messages: MailboxMessageSummary[] }
  | { status: "error"; message: string };

export type DetailState =
  | { status: "empty" }
  | { status: "loading" }
  | { status: "ready"; thread: MailboxThreadDetail }
  | { status: "error"; message: string };

export type SentMailboxState =
  | { status: "idle" }
  | { status: "loading" }
  | { status: "ready"; messages: OutboundMessageSummary[] }
  | { status: "error"; message: string };

export type OutboundDetailState =
  | { status: "empty" }
  | { status: "loading" }
  | {
      status: "ready";
      message: OutboundMessageDetail;
      thread: MailboxThreadDetail | null;
    }
  | { status: "error"; message: string };

export type MailboxMode = "inbox" | "sent";

export type InboxMailboxContentProps = {
  uploadApi: AttachmentUploadApi;
  onBack: () => void;
  contacts: Contact[];
  detailState: DetailState;
  onChangeMessageContact: (
    message: MailboxMessageDetail,
    contactId: string | null,
  ) => void;
  onReply: (
    message: MailboxMessageDetail,
    request: ReplyMessageRequest,
  ) => Promise<void>;
  onDownloadAttachment: (
    message: MailboxMessageDetail,
    attachmentId: string,
  ) => Promise<void>;
  onSelectMessage: (message: MailboxMessageSummary) => void;
  onToggleRead: (message: MailboxMessageSummary | MailboxMessageDetail) => void;
  state: MailboxState;
} & Partial<{
  selectedMessageId: string;
}>;

export type SentMailboxContentProps = {
  onBack: () => void;
  onSelectOutbound: (message: OutboundMessageSummary) => void;
  outboundDetailState: OutboundDetailState;
  sentState: SentMailboxState;
} & Partial<{
  selectedOutboundId: string;
}> &
  Omit<ThreadDetailProps, "thread">;

export type MailboxListProps = {
  messages: MailboxMessageSummary[];
} & Partial<{
  selectedMessageId: string;
  onSelectMessage: (message: MailboxMessageSummary) => void;
  onToggleRead: (message: MailboxMessageSummary) => void;
}>;

export type SentMessageListProps = {
  messages: OutboundMessageSummary[];
} & Partial<{
  selectedMessageId: string;
  onSelectMessage: (message: OutboundMessageSummary) => void;
}>;

export type ThreadDetailProps = {
  thread: MailboxThreadDetail;
} & Partial<{
  uploadApi: AttachmentUploadApi;
  contacts: Contact[];
  onToggleRead: (message: MailboxMessageDetail) => void;
  onContactChange: (
    message: MailboxMessageDetail,
    contactId: string | null,
  ) => void;
  onReply: (
    message: MailboxMessageDetail,
    request: ReplyMessageRequest,
  ) => Promise<void>;
  onDownloadAttachment: (
    message: MailboxMessageDetail,
    attachmentId: string,
  ) => Promise<void>;
}>;

export type MessageDetailProps = {
  message: MailboxMessageDetail;
} & Partial<{
  uploadApi: AttachmentUploadApi;
  contacts: Contact[];
  onToggleRead: (message: MailboxMessageDetail) => void;
  onContactChange: (
    message: MailboxMessageDetail,
    contactId: string | null,
  ) => void;
  onReply: (
    message: MailboxMessageDetail,
    request: ReplyMessageRequest,
  ) => Promise<void>;
  onDownloadAttachment: (
    message: MailboxMessageDetail,
    attachmentId: string,
  ) => Promise<void>;
}>;
