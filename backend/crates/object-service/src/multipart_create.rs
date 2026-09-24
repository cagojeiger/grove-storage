use std::future::Future;

/// Operations on an already-reserved native multipart upload.
pub trait MultipartCreate {
    type Error;

    /// None denotes a backend without a vendor multipart session.
    fn create_vendor_upload(
        &self,
    ) -> impl Future<Output = Result<Option<String>, Self::Error>> + Send;
    fn attach_vendor_upload(
        &self,
        upload_id: &str,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;
    fn prepare_relay(&self) -> impl Future<Output = Result<(), Self::Error>> + Send;
    /// Best-effort compensation reports its own failures without replacing the
    /// original error. An unknown vendor ID requires cleanup by object key.
    fn compensate(&self, upload_id: Option<&str>) -> impl Future<Output = ()> + Send;
}

/// Preserve the vendor handle across every fallible initialization step.
/// Cancellation recovery remains the responsibility of durable reservation scans.
pub async fn initialize<O: MultipartCreate>(operations: &O) -> Result<(), O::Error> {
    let upload_id = match operations.create_vendor_upload().await {
        Ok(upload_id) => upload_id,
        Err(error) => {
            operations.compensate(None).await;
            return Err(error);
        }
    };
    if let Some(upload_id) = &upload_id
        && let Err(error) = operations.attach_vendor_upload(upload_id).await
    {
        operations.compensate(Some(upload_id)).await;
        return Err(error);
    }
    if let Err(error) = operations.prepare_relay().await {
        operations.compensate(upload_id.as_deref()).await;
        return Err(error);
    }
    Ok(())
}
