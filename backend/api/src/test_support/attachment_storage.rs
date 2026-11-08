use super::*;
use async_trait::async_trait;
use shared::attachment_storage::{AttachmentStorage, SignedAttachmentUrl, StoredAttachment};
use shared::attachment_uploads::{
    AttachmentUploadGrant, AttachmentUploadService, CreateAttachmentUpload, UploadedAttachmentRef,
};
use shared::outbound::OutboundAttachmentInput;

#[derive(Default)]
pub struct TestAttachmentStorage {
    uploads:
        std::sync::Mutex<std::collections::BTreeMap<uuid::Uuid, (String, CreateAttachmentUpload)>>,
}

fn signed() -> SignedAttachmentUrl {
    SignedAttachmentUrl {
        url: "https://private-mail.s3.amazonaws.com/attachment?signature=test".into(),
        headers: Default::default(),
        expires_in_seconds: 900,
    }
}

#[async_trait]
impl AttachmentStorage for TestAttachmentStorage {
    async fn grant_upload(
        &self,
        _: &str,
        _: &CreateAttachmentUpload,
    ) -> AppResult<SignedAttachmentUrl> {
        Ok(signed())
    }
    async fn finalize_upload(
        &self,
        key: &str,
        request: &CreateAttachmentUpload,
    ) -> AppResult<StoredAttachment> {
        Ok(StoredAttachment {
            key: key.into(),
            version_id: "v1".into(),
            size_bytes: request.size_bytes,
        })
    }
    async fn read(&self, object: &StoredAttachment) -> AppResult<Vec<u8>> {
        Ok(vec![b'x'; object.size_bytes as usize])
    }
    async fn download(&self, _: &StoredAttachment, _: &str) -> AppResult<SignedAttachmentUrl> {
        Ok(signed())
    }
    async fn cache_download(&self, _: &str, _: Vec<u8>, _: &str) -> AppResult<SignedAttachmentUrl> {
        Ok(signed())
    }
}

#[async_trait]
impl AttachmentUploadService for TestAttachmentStorage {
    async fn create(
        &self,
        owner: &str,
        request: CreateAttachmentUpload,
    ) -> AppResult<AttachmentUploadGrant> {
        request.validate()?;
        let upload_id = uuid::Uuid::new_v4();
        self.uploads
            .lock()
            .unwrap()
            .insert(upload_id, (owner.into(), request));
        Ok(AttachmentUploadGrant {
            upload_id,
            upload: signed(),
        })
    }
    async fn resolve(
        &self,
        owner: &str,
        refs: &[UploadedAttachmentRef],
    ) -> AppResult<Vec<OutboundAttachmentInput>> {
        let uploads = self.uploads.lock().unwrap();
        refs.iter()
            .map(|reference| {
                let (subject, request) = uploads
                    .get(&reference.upload_id)
                    .filter(|(subject, _)| subject == owner)
                    .ok_or_else(|| AppError::Validation("unknown upload".into()))?;
                let _ = subject;
                Ok(OutboundAttachmentInput {
                    filename: request.filename.clone(),
                    content_type: request.content_type.clone(),
                    content_base64: String::new(),
                    storage: Some(StoredAttachment {
                        key: format!("attachments/files/{}", reference.upload_id),
                        version_id: "v1".into(),
                        size_bytes: request.size_bytes,
                    }),
                })
            })
            .collect()
    }
}
