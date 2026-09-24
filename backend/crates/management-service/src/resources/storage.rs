use crate::Error;
use filegate_db::{
    management::{AuditContext, IdentityTransaction},
    registry::{StorageRow, UpdateStorageOutcome},
};
use grove_management_command::{CommandName, Output};

pub(super) async fn write(
    tx: &mut IdentityTransaction<'_>,
    ctx: &AuditContext,
    name: CommandName,
    row: StorageRow,
) -> Result<Output, Error> {
    match name {
        CommandName::StorageCreate => {
            tx.create_resource_storage(ctx, &row).await?;
            Ok(Output::StorageCreate(super::reads::storage(row)?))
        }
        CommandName::StorageReplace => {
            match tx.replace_resource_storage(ctx, &row).await? {
                UpdateStorageOutcome::Updated => {}
                UpdateStorageOutcome::NotFound => return Err(Error::NotFound),
                UpdateStorageOutcome::LocationInUse => return Err(Error::Conflict),
            }
            Ok(Output::StorageReplace(super::reads::storage(row)?))
        }
        _ => Err(Error::RequestRejected),
    }
}
