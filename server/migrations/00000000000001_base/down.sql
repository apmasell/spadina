DROP INDEX bookmark_player_kind;
DROP INDEX localplayerchat_by_timestamp;
DROP INDEX localplayerchat_recipient;
DROP INDEX player_name;
DROP INDEX player_calendar_id;
DROP INDEX player_waiting_for_train;
DROP INDEX realm_asset;
DROP INDEX realm_in_directory;
DROP INDEX realm_owner_train;
DROP INDEX remoteplayerchat_by_timestamp;

DROP TABLE PublicKey;
DROP TABLE Bookmark;
DROP TABLE ServerACL;
DROP TABLE RealmCalendarSubscription;
DROP TABLE RealmAnnouncement;
DROP TABLE RealmChat;
DROP TABLE Realm;
DROP TABLE LocalPlayerChat;
DROP TABLE LocalPlayerLastRead;
DROP TABLE RemotePlayerChat;
DROP TABLE RemotePlayerLastRead;
DROP TABLE Player;
DROP TABLE RealmTrain;
DROP TABLE Announcement;
DROP TABLE BannedPeers;

DROP TABLE AuthOIDC;
DROP TABLE AuthOTP;

DROP FUNCTION gen_calendar_id;