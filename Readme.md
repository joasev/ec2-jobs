
Re-make of Tsunami to make it use the AWS SDK instead of Rusoto.
Tsuname was done for an old version of Rusoto that depends on a yanked library. Newer versions of Rusoto have a significant API changes which made it worth just migrating to the official AWS SKD instead.