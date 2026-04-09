local bluestation_type1 = Proto("bluestation_type1", "BlueStation Type-1 Capture")

local DIRECTION_NAMES = {
    [0] = "Downlink",
    [1] = "Uplink",
}

local LOGICAL_CHANNEL_NAMES = {
    [1] = "AACH",
    [2] = "BSCH",
    [3] = "BNCH",
    [4] = "SCH/HD",
    [5] = "SCH/F",
    [6] = "STCH",
    [7] = "SCH/HU",
    [8] = "TCH/S",
    [9] = "TCH/24",
    [10] = "TCH/48",
    [11] = "TCH/72",
    [12] = "BLCH",
    [13] = "CLCH",
}

local BLOCK_NAMES = {
    [0] = "Both",
    [1] = "Block1",
    [2] = "Block2",
    [255] = "Undefined",
}

local MAC_PDU_NAMES = {
    [0] = "MAC-RESOURCE / MAC-DATA",
    [1] = "MAC-FRAG / MAC-END",
    [2] = "Broadcast",
    [3] = "Supplementary / MAC-U-SIGNAL",
}

local BROADCAST_NAMES = {
    [0] = "SYSINFO",
    [1] = "ACCESS-DEFINE",
    [2] = "SYSINFO-DA",
}

local RESOURCE_ADDR_NAMES = {
    [0] = "NullPdu",
    [1] = "Ssi",
    [2] = "EventLabel",
    [3] = "Ussi",
    [4] = "Smi",
    [5] = "SsiAndEventLabel",
    [6] = "SsiAndUsageMarker",
    [7] = "SmiAndEventLabel",
}

local SYSINFO_OPT_NAMES = {
    [0] = "EvenMfDefForTsMode",
    [1] = "OddMfDefForTsMode",
    [2] = "DefaultDefForAccCodeA",
    [3] = "ExtServicesBroadcast",
}

local RES_REQ_NAMES = {
    [0] = "Req1Subslot",
    [1] = "Req1Slot",
    [2] = "Req2Slots",
    [3] = "Req3Slots",
    [4] = "Req4Slots",
    [5] = "Req5Slots",
    [6] = "Req6Slots",
    [7] = "Req8Slots",
    [8] = "Req10Slots",
    [9] = "Req13Slots",
    [10] = "Req17Slots",
    [11] = "Req24Slots",
    [12] = "Req34Slots",
    [13] = "Req51Slots",
    [14] = "Req68Slots",
    [15] = "ReqOver68",
}

local LLC_PDU_NAMES = {
    [0] = "BL-ADATA",
    [1] = "BL-DATA",
    [2] = "BL-UDATA",
    [3] = "BL-ACK",
    [4] = "BL-ADATA+FCS",
    [5] = "BL-DATA+FCS",
    [6] = "BL-UDATA+FCS",
    [7] = "BL-ACK+FCS",
}

local MLE_PROTOCOL_NAMES = {
    [1] = "MM",
    [2] = "CMCE",
    [4] = "SNDCP",
    [5] = "MLE",
    [6] = "TME",
}

local MLE_PDU_NAMES_DL = {
    [0] = "D-NEW CELL",
    [1] = "D-PREPARE FAIL",
    [2] = "D-NWRK-BROADCAST",
    [3] = "D-NWRK-BROADCAST EXT",
    [4] = "D-RESTORE ACK",
    [5] = "D-RESTORE FAIL",
    [6] = "D-CHANNEL RESPONSE",
    [7] = "MLE EXT PDU",
}

local MM_PDU_NAMES = {
    [0] = "D-OTAR",
    [1] = "D-AUTHENTICATION",
    [2] = "D-CK CHANGE DEMAND",
    [3] = "D-DISABLE",
    [4] = "D-ENABLE",
    [5] = "D-LOCATION UPDATE ACCEPT",
    [6] = "D-LOCATION UPDATE COMMAND",
    [7] = "D-LOCATION UPDATE REJECT",
    [9] = "D-LOCATION UPDATE PROCEEDING",
    [10] = "D-ATTACH/DETACH GROUP IDENTITY",
    [11] = "D-ATTACH/DETACH GROUP IDENTITY ACK",
    [12] = "D-MM STATUS",
    [15] = "MM FUNCTION NOT SUPPORTED",
}

local function is_traffic_channel(logical_channel)
    return logical_channel >= 8 and logical_channel <= 11
end

local MM_PDU_NAMES_UL = {
    [0] = "U-AUTHENTICATION",
    [1] = "U-ITSI DETACH",
    [2] = "U-LOCATION UPDATE DEMAND",
    [3] = "U-MM STATUS",
    [4] = "U-CK CHANGE RESULT",
    [5] = "U-OTAR",
    [6] = "U-INFORMATION PROVIDE",
    [7] = "U-ATTACH/DETACH GROUP IDENTITY",
    [8] = "U-ATTACH/DETACH GROUP IDENTITY ACK",
    [9] = "U-TEI PROVIDE",
    [11] = "U-DISABLE STATUS",
    [15] = "MM FUNCTION NOT SUPPORTED",
}

local CMCE_PDU_NAMES = {
    [0] = "D-ALERT",
    [1] = "D-CALL PROCEEDING",
    [2] = "D-CONNECT",
    [3] = "D-CONNECT ACKNOWLEDGE",
    [4] = "D-DISCONNECT",
    [5] = "D-INFO",
    [6] = "D-RELEASE",
    [7] = "D-SETUP",
    [8] = "D-STATUS",
    [9] = "D-TX CEASED",
    [10] = "D-TX CONTINUE",
    [11] = "D-TX GRANTED",
    [12] = "D-TX WAIT",
    [13] = "D-TX INTERRUPT",
    [14] = "D-CALL RESTORE",
    [15] = "D-SDS-DATA",
    [16] = "D-FACILITY",
    [31] = "CMCE FUNCTION NOT SUPPORTED",
}

local CMCE_PDU_NAMES_UL = {
    [0] = "U-ALERT",
    [2] = "U-CONNECT",
    [4] = "U-DISCONNECT",
    [5] = "U-INFO",
    [6] = "U-RELEASE",
    [7] = "U-SETUP",
    [8] = "U-STATUS",
    [9] = "U-TX CEASED",
    [10] = "U-TX DEMAND",
    [14] = "U-CALL RESTORE",
    [15] = "U-SDS-DATA",
    [16] = "U-FACILITY",
    [31] = "CMCE FUNCTION NOT SUPPORTED",
}

local LOCATION_UPDATE_TYPE_NAMES = {
    [0] = "RoamingLocationUpdating",
    [1] = "MigratingLocationUpdating",
    [2] = "PeriodicLocationUpdating",
    [3] = "ItsiAttach",
    [4] = "ServiceRestorationRoamingLocationUpdating",
    [5] = "ServiceRestorationMigratingLocationUpdating",
    [6] = "DemandLocationUpdating",
    [7] = "DisabledMsUpdating",
}

local LOCATION_UPDATE_ACCEPT_NAMES = {
    [0] = "RoamingLocationUpdating",
    [1] = "TemporaryRegistration",
    [2] = "PeriodicLocationUpdating",
    [3] = "ItsiAttach",
    [4] = "ServiceRestorationRoamingLocationUpdating",
    [5] = "MigratingOrServiceRestorationMigratingLocationUpdating",
    [6] = "DemandLocationUpdating",
    [7] = "DisabledMsUpdating",
}

local PARTY_TYPE_NAMES = {
    [0] = "SNA",
    [1] = "SSI",
    [2] = "TSI",
    [3] = "Reserved",
}

local CALL_TIMEOUT_NAMES = {
    [0] = "Infinite",
    [1] = "T30s",
    [2] = "T45s",
    [3] = "T60s",
    [4] = "T2m",
    [5] = "T3m",
    [6] = "T4m",
    [7] = "T5m",
    [8] = "T6m",
    [9] = "T8m",
    [10] = "T10m",
    [11] = "T12m",
    [12] = "T15m",
    [13] = "T20m",
    [14] = "T30m",
    [15] = "Reserved",
}

local CALL_TIMEOUT_SETUP_NAMES = {
    [0] = "Predefined",
    [1] = "T1s",
    [2] = "T2s",
    [3] = "T5s",
    [4] = "T10s",
    [5] = "T20s",
    [6] = "T30s",
    [7] = "T60s",
}

local CALL_STATUS_NAMES = {
    [0] = "CallProceeding",
    [1] = "CallQueued",
    [2] = "RequestedSubscriberPaged",
    [3] = "CallContinue",
    [4] = "HangtimeExpired",
}

local TRANSMISSION_GRANT_NAMES = {
    [0] = "Granted",
    [1] = "NotGranted",
    [2] = "RequestQueued",
    [3] = "GrantedToOtherUser",
}

local DISCONNECT_CAUSE_NAMES = {
    [0] = "CauseNotDefinedOrUnknown",
    [1] = "UserRequestedDisconnection",
    [2] = "CalledPartyBusy",
    [3] = "CalledPartyNotReachable",
    [4] = "CalledPartyDoesNotSupportEncryption",
    [5] = "CongestionInInfrastructure",
    [6] = "NotAllowedTrafficCase",
    [7] = "IncompatibleTrafficCase",
    [8] = "RequestedServiceNotAvailable",
    [9] = "PreEmptiveUseOfResource",
    [10] = "InvalidCallIdentifier",
    [11] = "CallRejectedByTheCalledParty",
    [12] = "NoIdleCcEntity",
    [13] = "ExpiryOfTimer",
    [14] = "SwmiRequestedDisconnection",
    [15] = "AcknowledgedServiceNotComplete",
    [16] = "UnknownTetraIdentity",
    [17] = "SsSpecificDisconnection",
    [18] = "UnknownExternalSubscriberIdentity",
    [19] = "CallRestorationOfTheOtherUserFailed",
    [20] = "CalledPartyRequiresEncryption",
    [21] = "ConcurrentSetUpNotSupported",
    [22] = "CalledPartyIsUnderTheSameDmGateOfTheCallingParty",
    [23] = "NonCallOwnerRequestedDisconnection",
}

local CMCE_TYPE3_NAMES = {
    [1] = "Dtmf",
    [2] = "ExternalSubscriberNumber",
    [3] = "Facility",
    [4] = "PollResponseAddr",
    [5] = "TempAddr",
    [6] = "DmMsAddr",
    [15] = "Proprietary",
}

local MM_TYPE34_NAMES = {
    [1] = "DefaultGroupAttachLifetime",
    [2] = "NewRegisteredArea",
    [3] = "SecurityDownlink",
    [4] = "GroupReportResponse",
    [5] = "GroupIdentityLocationAccept",
    [6] = "DmMsAddress",
    [7] = "GroupIdentityDownlink",
    [10] = "AuthenticationDownlink",
    [12] = "GroupIdentitySecurityRelatedInformation",
    [13] = "CellTypeControl",
    [15] = "Proprietary",
}

local STATUS_DOWNLINK_NAMES = {
    [1] = "ChangeOfEnergySavingModeRequest",
    [2] = "ChangeOfEnergySavingModeResponse",
    [3] = "DualWatchModeResponse",
    [4] = "TerminatingDualWatchModeResponse",
    [5] = "ChangeOfDualWatchModeRequest",
    [7] = "MsFrequencyBandsRequest",
    [8] = "PeriodicDistanceReporting",
    [16] = "AcceptanceToStartDmGatewayOperation",
    [17] = "RejectionToStartDmGatewayOperation",
    [18] = "AcceptanceToContinueDmGatewayOperation",
    [19] = "RejectionToContinueDmGatewayOperation",
    [20] = "AcceptanceToStopDmGatewayOperation",
    [21] = "AcceptanceOfDmMsAddresses",
    [22] = "CommandToRemoveDmMsAddresses",
    [23] = "CommandToChangeRegistrationLabel",
    [24] = "CommandToStopDmGatewayOperation",
}

local STATUS_UPLINK_NAMES = {
    [1] = "ChangeOfEnergySavingModeRequest",
    [2] = "ChangeOfEnergySavingModeResponse",
    [3] = "DualWatchModeRequest",
    [4] = "TerminatingDualWatchModeRequest",
    [5] = "ChangeOfDualWatchModeResponse",
    [6] = "StartOfDirectModeOperation",
    [7] = "MsFrequencyBandsInformation",
    [16] = "RequestToStartDmGatewayOperation",
    [17] = "RequestToContinuedmGatewayOperation",
    [18] = "RequestToStopDmGatewayOperation",
    [19] = "RequestToAddDmMsAddresses",
    [20] = "RequestToRemoveDmMsAddresses",
    [21] = "RequestToReplaceDmMsAddresses",
    [22] = "AcceptanceToRemovalOfDmMsAddresses",
    [23] = "AcceptanceToChangeRegistrationLabel",
    [24] = "AcceptanceToStopDmGatewayOperation",
}

local CIRCUIT_MODE_NAMES = {
    [0] = "Tch/S",
    [1] = "Tch/7.2",
    [2] = "Tch/4.8 N=1",
    [3] = "Tch/4.8 N=4",
    [4] = "Tch/4.8 N=8",
    [5] = "Tch/2.4 N=1",
    [6] = "Tch/2.4 N=4",
    [7] = "Tch/2.4 N=8",
}

local COMMUNICATION_TYPE_NAMES = {
    [0] = "P2p",
    [1] = "P2mp",
    [2] = "P2mpAcked",
    [3] = "Broadcast",
}

local ENERGY_SAVING_MODE_NAMES = {
    [0] = "StayAlive",
    [1] = "Eg1",
    [2] = "Eg2",
    [3] = "Eg3",
    [4] = "Eg4",
    [5] = "Eg5",
    [6] = "Eg6",
    [7] = "Eg7",
}

local SHORT_DATA_TYPE_NAMES = {
    [0] = "UserDefinedData-1",
    [1] = "UserDefinedData-2",
    [2] = "UserDefinedData-3",
    [3] = "UserDefinedData-4",
}

local SDS_PROTOCOL_ID_NAMES = {
    [1] = "Otak",
    [2] = "SimpleTextMessaging",
    [3] = "SimpleLocationSystem",
    [4] = "WirelessDatagramProtocol",
    [5] = "WirelessControlMessageProtocol",
    [6] = "MDmo",
    [7] = "PinAuth",
    [8] = "EteeMessage",
    [9] = "SimpleImmediateTextMessaging",
    [10] = "LocationInformationProtocol",
    [11] = "NetAssistProtocol2",
    [12] = "ConcatenatedSdsMessage",
    [13] = "Dotam",
    [14] = "SimpleAgnssService",
    [130] = "TextMessagingSdsTl",
    [131] = "LocationSystemSdsTl",
    [132] = "WirelessDatagramProtocolSdsTl",
    [133] = "WirelessControlMessageProtocolSdsTl",
    [134] = "MDmoSdsTl",
    [136] = "EteeMessageSdsTl",
    [137] = "ImmediateTextMessagingSdsTl",
    [138] = "MessageWithUserDataHeader",
    [140] = "ConcatenatedSdsMessageSdsTl",
    [141] = "AgnssServiceSdsTl",
}

local SHORT_REPORT_TYPE_NAMES = {
    [0] = "ProtOrEncodingNotSupported",
    [1] = "DestMemFull",
    [2] = "MessageReceived",
    [3] = "MessageConsumed",
}

local GROUP_IDENTITY_MODE_NAMES = {
    [0] = "Attach",
    [1] = "Detach",
}

local GROUP_IDENTITY_ACCEPT_REJECT_NAMES = {
    [0] = "Accept",
    [1] = "Reject",
}

local GROUP_IDENTITY_ATTACH_DETACH_MODE_UL_NAMES = {
    [0] = "Amendment",
    [1] = "DetachAllAndAttachSpecifiedGroups",
}

local GROUP_IDENTITY_ATTACHMENT_LIFETIME_NAMES = {
    [0] = "AttachmentNotNeeded",
    [1] = "AttachmentForNextItsiAttachRequired",
    [2] = "AttachmentNotAllowedForNextItsiAttach",
    [3] = "AttachmentForNextLocationUpdateRequired",
}

local GROUP_IDENTITY_ADDRESS_TYPE_NAMES = {
    [0] = "GSSI",
    [1] = "GSSI+Ext",
    [2] = "VGSSI",
    [3] = "GSSI+Ext+VGSSI",
}

local SPEECH_SERVICE_NAMES = {
    [0] = "TetraEncodedSpeech",
    [1] = "Reserved",
    [2] = "Reserved",
    [3] = "Proprietary",
}

local f = bluestation_type1.fields
f.magic = ProtoField.string("bluestation.type1.magic", "Magic")
f.version = ProtoField.uint8("bluestation.type1.version", "Version", base.DEC)
f.direction = ProtoField.uint8("bluestation.type1.direction", "Direction", base.DEC, DIRECTION_NAMES)
f.logical_channel = ProtoField.uint8("bluestation.type1.logical_channel", "Logical Channel", base.DEC, LOGICAL_CHANNEL_NAMES)
f.block = ProtoField.uint8("bluestation.type1.block", "Block", base.DEC, BLOCK_NAMES)
f.crc_pass = ProtoField.uint8("bluestation.type1.crc_pass", "CRC Pass", base.DEC, { [0] = "false", [1] = "true" })
f.reserved = ProtoField.uint8("bluestation.type1.reserved", "Reserved", base.HEX)
f.hyperframe = ProtoField.uint16("bluestation.type1.hyperframe", "Hyperframe", base.DEC)
f.multiframe = ProtoField.uint8("bluestation.type1.multiframe", "Multiframe", base.DEC)
f.frame = ProtoField.uint8("bluestation.type1.frame", "Frame", base.DEC)
f.timeslot = ProtoField.uint8("bluestation.type1.timeslot", "Timeslot", base.DEC)
f.scrambling_code = ProtoField.uint32("bluestation.type1.scrambling_code", "Scrambling Code", base.HEX)
f.bit_length = ProtoField.uint16("bluestation.type1.bit_length", "Bit Length", base.DEC)
f.bits = ProtoField.string("bluestation.type1.bits", "Type-1 Bits")

local pending_sch_hu_fragments = {}
local pending_sch_hu_summary_fragments = {}

local function lookup(tbl, value, fallback)
    if tbl[value] ~= nil then
        return tbl[value]
    end
    return fallback or tostring(value)
end

local function bool_text(value)
    return value == 1 and "true" or "false"
end

local function bits_to_uint(bits, start_bit, bit_len)
    if start_bit < 0 or bit_len < 0 then
        return nil
    end
    local stop = start_bit + bit_len
    if stop > #bits then
        return nil
    end

    local value = 0
    for idx = start_bit + 1, stop do
        local c = bits:byte(idx)
        if c ~= 48 and c ~= 49 then
            return nil
        end
        value = (value * 2) + (c - 48)
    end
    return value
end

local function bits_slice(bits, start_bit, bit_len)
    if start_bit < 0 or bit_len < 0 then
        return nil
    end
    local stop = start_bit + bit_len
    if stop > #bits then
        return nil
    end
    return bits:sub(start_bit + 1, stop)
end

local function preview_bits(bits, max_len)
    if bits == nil then
        return "<truncated>"
    end
    if #bits <= max_len then
        return bits
    end
    return bits:sub(1, max_len) .. "..."
end

local function bit_string_to_hex(bits)
    if bits == nil or bits == "" then
        return ""
    end
    local padded = bits
    local rem = #bits % 4
    if rem ~= 0 then
        padded = bits .. string.rep("0", 4 - rem)
    end

    local out = {}
    for idx = 1, #padded, 4 do
        local nibble = bits_to_uint(padded, idx - 1, 4) or 0
        out[#out + 1] = string.format("%X", nibble)
    end

    local suffix = rem ~= 0 and " (right-padded)" or ""
    return table.concat(out) .. suffix
end

local function format_uint_with_hex(value, bit_len)
    local width = math.max(1, math.floor((bit_len + 3) / 4))
    return string.format("%u (0x%0" .. tostring(width) .. "X)", value, value)
end

local function format_bits_value(bits)
    if bits == nil then
        return "<truncated>"
    end
    if bits == "" then
        return "<empty>"
    end
    if #bits <= 52 then
        local value = bits_to_uint(bits, 0, #bits)
        if value ~= nil then
            return format_uint_with_hex(value, #bits)
        end
    end
    return string.format("0x%s / bits=%s", bit_string_to_hex(bits), preview_bits(bits, 160))
end

local function add_text(tree, range, text)
    tree:add(range, text)
end

local function add_flag_summary(tree, range, label, flags)
    local enabled = {}
    for _, flag in ipairs(flags) do
        if flag.enabled then
            table.insert(enabled, flag.name)
        end
    end
    if #enabled == 0 then
        add_text(tree, range, label .. ": none")
    else
        add_text(tree, range, label .. ": " .. table.concat(enabled, ", "))
    end
end

local function add_named_value(tree, range, label, value, names, bit_len)
    add_text(tree, range, string.format("%s: %s [%s]", label, lookup(names, value, tostring(value)), format_uint_with_hex(value, bit_len)))
end

local function new_cursor(bits)
    return {
        bits = bits or "",
        pos = 0,
    }
end

local function cursor_remaining(cur)
    return #cur.bits - cur.pos
end

local function cursor_peek_uint(cur, offset, bit_len)
    return bits_to_uint(cur.bits, cur.pos + offset, bit_len)
end

local function cursor_read_uint(cur, bit_len)
    local value = bits_to_uint(cur.bits, cur.pos, bit_len)
    if value == nil then
        return nil
    end
    cur.pos = cur.pos + bit_len
    return value
end

local function cursor_read_bits(cur, bit_len)
    local value = bits_slice(cur.bits, cur.pos, bit_len)
    if value == nil then
        return nil
    end
    cur.pos = cur.pos + bit_len
    return value
end

local function read_obit(cur, tree, range, context)
    local obit = cursor_read_uint(cur, 1)
    if obit == nil then
        add_text(tree, range, context .. ": missing o-bit")
        return nil
    end
    return obit == 1
end

local function finalize_optional_tail(cur, tree, range, context)
    local trailing = cursor_read_uint(cur, 1)
    if trailing == nil then
        add_text(tree, range, context .. ": missing trailing m-bit")
        return
    end
    if trailing ~= 0 then
        add_text(tree, range, context .. ": unexpected trailing m-bit=" .. trailing)
    end
    if cursor_remaining(cur) > 0 then
        local tail = cursor_read_bits(cur, cursor_remaining(cur))
        add_text(tree, range, string.format("%s trailing bits (%u): %s", context, #tail, preview_bits(tail, 160)))
    end
end

local function parse_type2_uint(cur, tree, range, label, bit_len, names)
    local pbit = cursor_read_uint(cur, 1)
    if pbit == nil then
        add_text(tree, range, label .. ": missing p-bit")
        return nil
    end
    if pbit == 0 then
        return nil
    end
    local value = cursor_read_uint(cur, bit_len)
    if value == nil then
        add_text(tree, range, label .. ": truncated")
        return nil
    end
    if names ~= nil then
        add_named_value(tree, range, label, value, names, bit_len)
    else
        add_text(tree, range, label .. ": " .. format_uint_with_hex(value, bit_len))
    end
    return value
end

local function parse_type2_struct(cur, tree, range, label, parser)
    local pbit = cursor_read_uint(cur, 1)
    if pbit == nil then
        add_text(tree, range, label .. ": missing p-bit")
        return false
    end
    if pbit == 0 then
        return false
    end
    local sub = tree:add(range, label)
    parser(cur, sub, range)
    return true
end

local function peek_type34(cur, expected_id)
    local mbit = cursor_peek_uint(cur, 0, 1)
    if mbit == nil then
        return nil
    end
    if mbit == 0 then
        return false
    end
    local field_id = cursor_peek_uint(cur, 1, 4)
    if field_id == nil then
        return nil
    end
    return field_id == expected_id
end

local function parse_type3_generic(cur, tree, range, expected_id, label, id_names)
    local present = peek_type34(cur, expected_id)
    if present == nil then
        add_text(tree, range, label .. ": missing type-3 header")
        return nil
    end
    if not present then
        return nil
    end

    cursor_read_uint(cur, 1)
    local field_id = cursor_read_uint(cur, 4)
    local len_bits = cursor_read_uint(cur, 11)
    if len_bits == nil then
        add_text(tree, range, label .. ": missing length")
        return nil
    end
    local data_bits = cursor_read_bits(cur, len_bits)
    if data_bits == nil then
        add_text(tree, range, label .. ": truncated payload")
        return nil
    end

    local field_name = lookup(id_names or {}, field_id, "Field " .. tostring(field_id))
    local sub = tree:add(range, string.format("%s (%u bits)", label or field_name, len_bits))
    add_text(sub, range, "Element ID: " .. field_name)
    add_text(sub, range, "Value: " .. format_bits_value(data_bits))
    return {
        field_id = field_id,
        len_bits = len_bits,
        data_bits = data_bits,
    }
end

local function parse_type3_struct(cur, tree, range, expected_id, label, parser)
    local present = peek_type34(cur, expected_id)
    if present == nil then
        add_text(tree, range, label .. ": missing type-3 header")
        return false
    end
    if not present then
        return false
    end

    cursor_read_uint(cur, 1)
    local field_id = cursor_read_uint(cur, 4)
    local len_bits = cursor_read_uint(cur, 11)
    if len_bits == nil then
        add_text(tree, range, label .. ": missing length")
        return false
    end
    local data_bits = cursor_read_bits(cur, len_bits)
    if data_bits == nil then
        add_text(tree, range, label .. ": truncated payload")
        return false
    end

    local sub = tree:add(range, string.format("%s (%u bits)", label, len_bits))
    add_text(sub, range, "Element ID: " .. lookup(MM_TYPE34_NAMES, field_id, lookup(CMCE_TYPE3_NAMES, field_id, tostring(field_id))))

    local subcur = new_cursor(data_bits)
    parser(subcur, sub, range)
    if cursor_remaining(subcur) > 0 then
        local tail = cursor_read_bits(subcur, cursor_remaining(subcur))
        add_text(sub, range, string.format("Struct trailing bits (%u): %s", #tail, preview_bits(tail, 160)))
    end
    return true
end

local function parse_type4_generic(cur, tree, range, expected_id, label, id_names)
    local present = peek_type34(cur, expected_id)
    if present == nil then
        add_text(tree, range, label .. ": missing type-4 header")
        return nil
    end
    if not present then
        return nil
    end

    cursor_read_uint(cur, 1)
    local field_id = cursor_read_uint(cur, 4)
    local len_bits = cursor_read_uint(cur, 11)
    local elem_count = cursor_read_uint(cur, 6)
    if len_bits == nil or elem_count == nil then
        add_text(tree, range, label .. ": truncated type-4 header")
        return nil
    end
    if len_bits < 6 then
        add_text(tree, range, label .. ": invalid type-4 length=" .. len_bits)
        return nil
    end
    local payload_len = len_bits - 6
    local data_bits = cursor_read_bits(cur, payload_len)
    if data_bits == nil then
        add_text(tree, range, label .. ": truncated payload")
        return nil
    end

    local field_name = lookup(id_names or {}, field_id, "Field " .. tostring(field_id))
    local sub = tree:add(range, string.format("%s (%u elems, %u bits)", label or field_name, elem_count, payload_len))
    add_text(sub, range, "Element ID: " .. field_name)
    add_text(sub, range, "Value: " .. format_bits_value(data_bits))
    return {
        field_id = field_id,
        len_bits = payload_len,
        elems = elem_count,
        data_bits = data_bits,
    }
end

local function parse_type4_struct(cur, tree, range, expected_id, label, parser)
    local present = peek_type34(cur, expected_id)
    if present == nil then
        add_text(tree, range, label .. ": missing type-4 header")
        return false
    end
    if not present then
        return false
    end

    cursor_read_uint(cur, 1)
    local field_id = cursor_read_uint(cur, 4)
    local len_bits = cursor_read_uint(cur, 11)
    local elem_count = cursor_read_uint(cur, 6)
    if len_bits == nil or elem_count == nil then
        add_text(tree, range, label .. ": truncated type-4 header")
        return false
    end
    if len_bits < 6 then
        add_text(tree, range, label .. ": invalid type-4 length=" .. len_bits)
        return false
    end

    local payload_len = len_bits - 6
    local payload_bits = cursor_read_bits(cur, payload_len)
    if payload_bits == nil then
        add_text(tree, range, label .. ": truncated payload")
        return false
    end

    local sub = tree:add(range, string.format("%s (%u elems, %u bits)", label, elem_count, payload_len))
    add_text(sub, range, "Element ID: " .. lookup(MM_TYPE34_NAMES, field_id, tostring(field_id)))

    local payload_cur = new_cursor(payload_bits)
    for idx = 1, elem_count do
        local elem_tree = sub:add(range, "Element " .. idx)
        parser(payload_cur, elem_tree, range)
    end

    if cursor_remaining(payload_cur) > 0 then
        local tail = cursor_read_bits(payload_cur, cursor_remaining(payload_cur))
        add_text(sub, range, string.format("Type-4 trailing bits (%u): %s", #tail, preview_bits(tail, 160)))
    end
    return true
end

local function parse_basic_service_information(cur, tree, range)
    local circuit_mode = cursor_read_uint(cur, 3)
    local encryption_flag = cursor_read_uint(cur, 1)
    local communication_type = cursor_read_uint(cur, 2)
    if circuit_mode == nil or encryption_flag == nil or communication_type == nil then
        add_text(tree, range, "Basic service information: truncated")
        return
    end

    add_named_value(tree, range, "Circuit mode", circuit_mode, CIRCUIT_MODE_NAMES, 3)
    add_text(tree, range, "Encryption flag: " .. bool_text(encryption_flag))
    add_named_value(tree, range, "Communication type", communication_type, COMMUNICATION_TYPE_NAMES, 2)

    if circuit_mode == 0 then
        local speech_service = cursor_read_uint(cur, 2)
        if speech_service == nil then
            add_text(tree, range, "Speech service: truncated")
            return
        end
        add_named_value(tree, range, "Speech service", speech_service, SPEECH_SERVICE_NAMES, 2)
    else
        local slots_per_frame = cursor_read_uint(cur, 2)
        if slots_per_frame == nil then
            add_text(tree, range, "Slots per frame: truncated")
            return
        end
        add_text(tree, range, string.format("Slots per frame: %u", slots_per_frame + 1))
    end
end

local function parse_energy_saving_information(cur, tree, range)
    local mode = cursor_read_uint(cur, 3)
    local frame_number = cursor_read_uint(cur, 2)
    local multiframe_number = cursor_read_uint(cur, 2)
    if mode == nil or frame_number == nil or multiframe_number == nil then
        add_text(tree, range, "Energy saving information: truncated")
        return
    end

    add_named_value(tree, range, "Energy saving mode", mode, ENERGY_SAVING_MODE_NAMES, 3)
    add_text(tree, range, "Frame number: " .. frame_number)
    add_text(tree, range, "Multiframe number: " .. multiframe_number)
end

local function parse_group_identity_attachment(cur, tree, range)
    local lifetime = cursor_read_uint(cur, 2)
    local class_of_usage = cursor_read_uint(cur, 3)
    if lifetime == nil or class_of_usage == nil then
        add_text(tree, range, "Group identity attachment: truncated")
        return
    end

    add_named_value(tree, range, "Attachment lifetime", lifetime, GROUP_IDENTITY_ATTACHMENT_LIFETIME_NAMES, 2)
    add_text(tree, range, "Class of usage: " .. class_of_usage)
end

local function parse_group_identity_uplink(cur, tree, range)
    local attach_detach_type = cursor_read_uint(cur, 1)
    if attach_detach_type == nil then
        add_text(tree, range, "Group identity uplink: truncated")
        return
    end

    if attach_detach_type == 0 then
        local class_of_usage = cursor_read_uint(cur, 3)
        if class_of_usage == nil then
            add_text(tree, range, "Class of usage: truncated")
            return
        end
        add_text(tree, range, "Class of usage: " .. class_of_usage)
    else
        local detachment = cursor_read_uint(cur, 2)
        if detachment == nil then
            add_text(tree, range, "Group identity detachment uplink: truncated")
            return
        end
        add_text(tree, range, "Group identity detachment uplink: " .. detachment)
    end

    local address_type = cursor_read_uint(cur, 2)
    if address_type == nil then
        add_text(tree, range, "Group identity address type: truncated")
        return
    end
    add_named_value(tree, range, "Group identity address type", address_type, GROUP_IDENTITY_ADDRESS_TYPE_NAMES, 2)

    if address_type == 0 or address_type == 1 then
        local gssi = cursor_read_uint(cur, 24)
        if gssi == nil then
            add_text(tree, range, "GSSI: truncated")
            return
        end
        add_text(tree, range, "GSSI: " .. format_uint_with_hex(gssi, 24))
    end
    if address_type == 1 then
        local address_extension = cursor_read_uint(cur, 24)
        if address_extension == nil then
            add_text(tree, range, "Address extension: truncated")
            return
        end
        add_text(tree, range, "Address extension: " .. format_uint_with_hex(address_extension, 24))
    end
    if address_type == 2 then
        local vgssi = cursor_read_uint(cur, 24)
        if vgssi == nil then
            add_text(tree, range, "VGSSI: truncated")
            return
        end
        add_text(tree, range, "VGSSI: " .. format_uint_with_hex(vgssi, 24))
    end
end

local function parse_group_identity_location_demand(cur, tree, range)
    local reserved = cursor_read_uint(cur, 1)
    local mode = cursor_read_uint(cur, 1)
    if reserved == nil or mode == nil then
        add_text(tree, range, "Group identity location demand: truncated")
        return
    end

    add_text(tree, range, "Reserved: " .. reserved)
    add_named_value(tree, range, "Group identity attach/detach mode", mode, GROUP_IDENTITY_ATTACH_DETACH_MODE_UL_NAMES, 1)

    local obit = read_obit(cur, tree, range, "Group identity location demand")
    if obit == nil then
        return
    end
    if obit then
        parse_type4_struct(cur, tree, range, 8, "Group identity uplink", parse_group_identity_uplink)
        finalize_optional_tail(cur, tree, range, "Group identity location demand")
    end
end

local function parse_group_identity_downlink(cur, tree, range)
    local attach_detach_type = cursor_read_uint(cur, 1)
    if attach_detach_type == nil then
        add_text(tree, range, "Group identity downlink: truncated")
        return
    end

    if attach_detach_type == 0 then
        local attachment_tree = tree:add(range, "Group identity attachment")
        parse_group_identity_attachment(cur, attachment_tree, range)
    else
        local detachment_uplink = cursor_read_uint(cur, 2)
        if detachment_uplink == nil then
            add_text(tree, range, "Group identity detachment uplink: truncated")
            return
        end
        add_text(tree, range, "Group identity detachment uplink: " .. detachment_uplink)
    end

    local address_type = cursor_read_uint(cur, 2)
    if address_type == nil then
        add_text(tree, range, "Group identity address type: truncated")
        return
    end
    add_named_value(tree, range, "Group identity address type", address_type, GROUP_IDENTITY_ADDRESS_TYPE_NAMES, 2)

    if address_type == 0 or address_type == 1 or address_type == 3 then
        local gssi = cursor_read_uint(cur, 24)
        if gssi == nil then
            add_text(tree, range, "GSSI: truncated")
            return
        end
        add_text(tree, range, "GSSI: " .. format_uint_with_hex(gssi, 24))
    end
    if address_type == 1 or address_type == 3 then
        local address_extension = cursor_read_uint(cur, 24)
        if address_extension == nil then
            add_text(tree, range, "Address extension: truncated")
            return
        end
        add_text(tree, range, "Address extension: " .. format_uint_with_hex(address_extension, 24))
    end
    if address_type == 2 or address_type == 3 then
        local vgssi = cursor_read_uint(cur, 24)
        if vgssi == nil then
            add_text(tree, range, "VGSSI: truncated")
            return
        end
        add_text(tree, range, "VGSSI: " .. format_uint_with_hex(vgssi, 24))
    end
end

local function parse_group_identity_location_accept(cur, tree, range)
    local accept_reject = cursor_read_uint(cur, 1)
    local reserved = cursor_read_uint(cur, 1)
    if accept_reject == nil or reserved == nil then
        add_text(tree, range, "Group identity location accept: truncated")
        return
    end

    add_named_value(tree, range, "Accept/reject", accept_reject, GROUP_IDENTITY_ACCEPT_REJECT_NAMES, 1)
    add_text(tree, range, "Reserved: " .. reserved)

    local obit = read_obit(cur, tree, range, "Group identity location accept")
    if obit == nil then
        return
    end

    if obit then
        parse_type4_struct(cur, tree, range, 7, "Group identity downlink", parse_group_identity_downlink)
        finalize_optional_tail(cur, tree, range, "Group identity location accept")
    end
end

local function describe_pre_coded_status(value)
    if value == 0 then
        return "Emergency"
    end
    if value >= 1 and value <= 31742 then
        return "Reserved(" .. value .. ")"
    end
    if value >= 31743 and value <= 32767 then
        local short_report_type = math.floor(value / 256) % 4
        local message_reference = value % 256
        return string.format("SdsTl(%s, message_reference=%u)", lookup(SHORT_REPORT_TYPE_NAMES, short_report_type, tostring(short_report_type)), message_reference)
    end
    return "NetworkUserSpecific(" .. value .. ")"
end

local function parse_sds_payload(bits, tree, range)
    if bits == nil or bits == "" then
        add_text(tree, range, "SDS payload: <empty>")
        return
    end

    add_text(tree, range, string.format("Payload bits (%u): %s", #bits, preview_bits(bits, 160)))
    add_text(tree, range, "Payload hex: 0x" .. bit_string_to_hex(bits))

    if #bits >= 8 then
        local protocol_id = bits_to_uint(bits, 0, 8)
        if protocol_id ~= nil then
            add_named_value(tree, range, "SDS protocol id", protocol_id, SDS_PROTOCOL_ID_NAMES, 8)
        end
    end
end

local function parse_type34_tail_only(cur, tree, range, label)
    if cursor_remaining(cur) > 0 then
        local tail = cursor_read_bits(cur, cursor_remaining(cur))
        add_text(tree, range, string.format("%s raw tail (%u bits): %s", label, #tail, preview_bits(tail, 160)))
    end
end

local function parse_mle_mm_downlink(bits, tree, range)
    local cur = new_cursor(bits)
    local pdu_type = cursor_read_uint(cur, 4)
    if pdu_type == nil then
        add_text(tree, range, "MM PDU: truncated")
        return
    end

    local mm_tree = tree:add(range, "MM: " .. lookup(MM_PDU_NAMES, pdu_type, "Unknown"))
    add_named_value(mm_tree, range, "PDU type", pdu_type, MM_PDU_NAMES, 4)

    if pdu_type == 5 then
        local location_update_accept_type = cursor_read_uint(cur, 3)
        if location_update_accept_type == nil then
            add_text(mm_tree, range, "Location update accept type: truncated")
            return
        end
        add_named_value(mm_tree, range, "Location update accept type", location_update_accept_type, LOCATION_UPDATE_ACCEPT_NAMES, 3)

        local obit = read_obit(cur, mm_tree, range, "D-LOCATION UPDATE ACCEPT")
        if obit == nil then
            return
        end

        if obit then
            parse_type2_uint(cur, mm_tree, range, "SSI", 24)
            parse_type2_uint(cur, mm_tree, range, "Address extension", 24)
            parse_type2_uint(cur, mm_tree, range, "Subscriber class", 16)
            parse_type2_struct(cur, mm_tree, range, "Energy saving information", parse_energy_saving_information)
            parse_type2_uint(cur, mm_tree, range, "SCCH information and distribution on 18th frame", 6)
            parse_type4_generic(cur, mm_tree, range, 2, "New registered area", MM_TYPE34_NAMES)
            parse_type3_generic(cur, mm_tree, range, 3, "Security downlink", MM_TYPE34_NAMES)
            parse_type3_struct(cur, mm_tree, range, 5, "Group identity location accept", parse_group_identity_location_accept)
            parse_type3_generic(cur, mm_tree, range, 1, "Default group attachment lifetime", MM_TYPE34_NAMES)
            parse_type3_generic(cur, mm_tree, range, 10, "Authentication downlink", MM_TYPE34_NAMES)
            parse_type4_generic(cur, mm_tree, range, 12, "Group identity security related information", MM_TYPE34_NAMES)
            parse_type3_generic(cur, mm_tree, range, 13, "Cell type control", MM_TYPE34_NAMES)
            parse_type3_generic(cur, mm_tree, range, 15, "Proprietary", MM_TYPE34_NAMES)
            finalize_optional_tail(cur, mm_tree, range, "D-LOCATION UPDATE ACCEPT")
        end
    elseif pdu_type == 6 then
        local group_identity_report = cursor_read_uint(cur, 1)
        local cipher_control = cursor_read_uint(cur, 1)
        if group_identity_report == nil or cipher_control == nil then
            add_text(mm_tree, range, "D-LOCATION UPDATE COMMAND: truncated")
            return
        end
        add_text(mm_tree, range, "Group identity report: " .. bool_text(group_identity_report))
        add_text(mm_tree, range, "Cipher control: " .. bool_text(cipher_control))
        if cipher_control == 1 then
            local ciphering_parameters = cursor_read_uint(cur, 10)
            if ciphering_parameters == nil then
                add_text(mm_tree, range, "Ciphering parameters: truncated")
                return
            end
            add_text(mm_tree, range, "Ciphering parameters: " .. format_uint_with_hex(ciphering_parameters, 10))
        end

        local obit = read_obit(cur, mm_tree, range, "D-LOCATION UPDATE COMMAND")
        if obit == nil then
            return
        end
        if obit then
            parse_type2_uint(cur, mm_tree, range, "Address extension", 24)
            parse_type34_tail_only(cur, mm_tree, range, "D-LOCATION UPDATE COMMAND optional tail")
        end
    elseif pdu_type == 7 then
        local location_update_type = cursor_read_uint(cur, 3)
        local reject_cause = cursor_read_uint(cur, 5)
        local cipher_control = cursor_read_uint(cur, 1)
        if location_update_type == nil or reject_cause == nil or cipher_control == nil then
            add_text(mm_tree, range, "D-LOCATION UPDATE REJECT: truncated")
            return
        end
        add_named_value(mm_tree, range, "Location update type", location_update_type, LOCATION_UPDATE_TYPE_NAMES, 3)
        add_text(mm_tree, range, "Reject cause: " .. format_uint_with_hex(reject_cause, 5))
        add_text(mm_tree, range, "Cipher control: " .. bool_text(cipher_control))
        if cipher_control == 1 then
            local ciphering_parameters = cursor_read_uint(cur, 10)
            if ciphering_parameters == nil then
                add_text(mm_tree, range, "Ciphering parameters: truncated")
                return
            end
            add_text(mm_tree, range, "Ciphering parameters: " .. format_uint_with_hex(ciphering_parameters, 10))
        end

        local obit = read_obit(cur, mm_tree, range, "D-LOCATION UPDATE REJECT")
        if obit == nil then
            return
        end
        if obit then
            parse_type2_uint(cur, mm_tree, range, "Address extension", 24)
            parse_type3_generic(cur, mm_tree, range, 13, "Cell type control", MM_TYPE34_NAMES)
            parse_type3_generic(cur, mm_tree, range, 15, "Proprietary", MM_TYPE34_NAMES)
            finalize_optional_tail(cur, mm_tree, range, "D-LOCATION UPDATE REJECT")
        end
    elseif pdu_type == 9 then
        local ssi = cursor_read_uint(cur, 24)
        local address_extension = cursor_read_uint(cur, 24)
        if ssi == nil or address_extension == nil then
            add_text(mm_tree, range, "D-LOCATION UPDATE PROCEEDING: truncated")
            return
        end
        add_text(mm_tree, range, "SSI: " .. format_uint_with_hex(ssi, 24))
        add_text(mm_tree, range, "Address extension: " .. format_uint_with_hex(address_extension, 24))

        local obit = read_obit(cur, mm_tree, range, "D-LOCATION UPDATE PROCEEDING")
        if obit == nil then
            return
        end
        if obit then
            parse_type3_generic(cur, mm_tree, range, 15, "Proprietary", MM_TYPE34_NAMES)
            finalize_optional_tail(cur, mm_tree, range, "D-LOCATION UPDATE PROCEEDING")
        end
    elseif pdu_type == 10 then
        local report = cursor_read_uint(cur, 1)
        local ack_req = cursor_read_uint(cur, 1)
        local mode = cursor_read_uint(cur, 1)
        if report == nil or ack_req == nil or mode == nil then
            add_text(mm_tree, range, "D-ATTACH/DETACH GROUP IDENTITY: truncated")
            return
        end
        add_text(mm_tree, range, "Group identity report: " .. bool_text(report))
        add_text(mm_tree, range, "Group identity acknowledgement request: " .. bool_text(ack_req))
        add_named_value(mm_tree, range, "Attach/detach mode", mode, GROUP_IDENTITY_MODE_NAMES, 1)

        local obit = read_obit(cur, mm_tree, range, "D-ATTACH/DETACH GROUP IDENTITY")
        if obit == nil then
            return
        end
        if obit then
            parse_type3_generic(cur, mm_tree, range, 15, "Proprietary", MM_TYPE34_NAMES)
            parse_type3_generic(cur, mm_tree, range, 4, "Group report response", MM_TYPE34_NAMES)
            parse_type4_struct(cur, mm_tree, range, 7, "Group identity downlink", parse_group_identity_downlink)
            parse_type4_generic(cur, mm_tree, range, 12, "Group identity security related information", MM_TYPE34_NAMES)
            finalize_optional_tail(cur, mm_tree, range, "D-ATTACH/DETACH GROUP IDENTITY")
        end
    elseif pdu_type == 12 then
        local status_downlink = cursor_read_uint(cur, 6)
        if status_downlink == nil then
            add_text(mm_tree, range, "D-MM STATUS: truncated")
            return
        end
        if STATUS_DOWNLINK_NAMES[status_downlink] ~= nil then
            add_named_value(mm_tree, range, "Status downlink", status_downlink, STATUS_DOWNLINK_NAMES, 6)
        else
            add_text(mm_tree, range, "Status downlink: " .. format_uint_with_hex(status_downlink, 6))
        end
        parse_type34_tail_only(cur, mm_tree, range, "D-MM STATUS dependent information")
    else
        parse_type34_tail_only(cur, mm_tree, range, "MM payload")
    end
end

local function parse_mle_mm_uplink(bits, tree, range)
    local cur = new_cursor(bits)
    local pdu_type = cursor_read_uint(cur, 4)
    if pdu_type == nil then
        add_text(tree, range, "MM PDU: truncated")
        return
    end

    local mm_tree = tree:add(range, "MM: " .. lookup(MM_PDU_NAMES_UL, pdu_type, "Unknown"))
    add_named_value(mm_tree, range, "PDU type", pdu_type, MM_PDU_NAMES_UL, 4)

    if pdu_type == 1 then
        local obit = read_obit(cur, mm_tree, range, "U-ITSI DETACH")
        if obit == nil then
            return
        end
        if obit then
            parse_type2_uint(cur, mm_tree, range, "Address extension", 24)
            parse_type3_generic(cur, mm_tree, range, 15, "Proprietary", MM_TYPE34_NAMES)
            finalize_optional_tail(cur, mm_tree, range, "U-ITSI DETACH")
        end
    elseif pdu_type == 2 then
        local location_update_type = cursor_read_uint(cur, 3)
        local request_to_append_la = cursor_read_uint(cur, 1)
        local cipher_control = cursor_read_uint(cur, 1)
        if location_update_type == nil or request_to_append_la == nil or cipher_control == nil then
            add_text(mm_tree, range, "U-LOCATION UPDATE DEMAND: truncated")
            return
        end
        add_named_value(mm_tree, range, "Location update type", location_update_type, LOCATION_UPDATE_TYPE_NAMES, 3)
        add_text(mm_tree, range, "Request to append LA: " .. bool_text(request_to_append_la))
        add_text(mm_tree, range, "Cipher control: " .. bool_text(cipher_control))
        if cipher_control == 1 then
            local ciphering_parameters = cursor_read_uint(cur, 10)
            if ciphering_parameters == nil then
                add_text(mm_tree, range, "Ciphering parameters: truncated")
                return
            end
            add_text(mm_tree, range, "Ciphering parameters: " .. format_uint_with_hex(ciphering_parameters, 10))
        end

        local obit = read_obit(cur, mm_tree, range, "U-LOCATION UPDATE DEMAND")
        if obit == nil then
            return
        end
        if obit then
            parse_type2_uint(cur, mm_tree, range, "Class of MS", 24)
            parse_type2_uint(cur, mm_tree, range, "Energy saving mode", 3, ENERGY_SAVING_MODE_NAMES)
            parse_type2_uint(cur, mm_tree, range, "LA information", 15)
            parse_type2_uint(cur, mm_tree, range, "SSI", 24)
            parse_type2_uint(cur, mm_tree, range, "Address extension", 24)
            parse_type3_struct(cur, mm_tree, range, 8, "Group identity location demand", parse_group_identity_location_demand)
            parse_type3_generic(cur, mm_tree, range, 4, "Group report response", MM_TYPE34_NAMES)
            parse_type3_generic(cur, mm_tree, range, 9, "Authentication uplink", MM_TYPE34_NAMES)
            parse_type3_generic(cur, mm_tree, range, 11, "Extended capabilities", MM_TYPE34_NAMES)
            parse_type3_generic(cur, mm_tree, range, 15, "Proprietary", MM_TYPE34_NAMES)
            finalize_optional_tail(cur, mm_tree, range, "U-LOCATION UPDATE DEMAND")
        end
    elseif pdu_type == 3 then
        local status_uplink = cursor_read_uint(cur, 6)
        if status_uplink == nil then
            add_text(mm_tree, range, "U-MM STATUS: truncated")
            return
        end
        if STATUS_UPLINK_NAMES[status_uplink] ~= nil then
            add_named_value(mm_tree, range, "Status uplink", status_uplink, STATUS_UPLINK_NAMES, 6)
        else
            add_text(mm_tree, range, "Status uplink: " .. format_uint_with_hex(status_uplink, 6))
        end
        if cursor_remaining(cur) > 0 then
            local tail = cursor_read_bits(cur, cursor_remaining(cur))
            add_text(mm_tree, range, string.format("U-MM STATUS dependent information (%u bits): %s", #tail, preview_bits(tail, 160)))
        end
    elseif pdu_type == 7 or pdu_type == 8 then
        local group_identity_report = cursor_read_uint(cur, 1)
        local mode = cursor_read_uint(cur, 1)
        if group_identity_report == nil or mode == nil then
            add_text(mm_tree, range, "U-ATTACH/DETACH GROUP IDENTITY: truncated")
            return
        end
        add_text(mm_tree, range, "Group identity report: " .. bool_text(group_identity_report))
        add_named_value(mm_tree, range, "Group identity attach/detach mode", mode, GROUP_IDENTITY_ATTACH_DETACH_MODE_UL_NAMES, 1)

        local obit = read_obit(cur, mm_tree, range, lookup(MM_PDU_NAMES_UL, pdu_type, "MM"))
        if obit == nil then
            return
        end
        if obit then
            parse_type3_generic(cur, mm_tree, range, 4, "Group report response", MM_TYPE34_NAMES)
            parse_type4_struct(cur, mm_tree, range, 8, "Group identity uplink", parse_group_identity_uplink)
            parse_type3_generic(cur, mm_tree, range, 15, "Proprietary", MM_TYPE34_NAMES)
            finalize_optional_tail(cur, mm_tree, range, lookup(MM_PDU_NAMES_UL, pdu_type, "MM"))
        end
    else
        parse_type34_tail_only(cur, mm_tree, range, "MM payload")
    end
end

local function parse_cmce_setup_optional_calling_party(cur, tree, range)
    local pbit = cursor_read_uint(cur, 1)
    if pbit == nil then
        add_text(tree, range, "Calling party type identifier: missing p-bit")
        return
    end
    if pbit == 0 then
        return
    end

    local cpti = cursor_read_uint(cur, 2)
    if cpti == nil then
        add_text(tree, range, "Calling party type identifier: truncated")
        return
    end
    add_named_value(tree, range, "Calling party type identifier", cpti, PARTY_TYPE_NAMES, 2)

    if cpti == 1 or cpti == 2 then
        local ssi = cursor_read_uint(cur, 24)
        if ssi == nil then
            add_text(tree, range, "Calling party SSI: truncated")
            return
        end
        add_text(tree, range, "Calling party SSI: " .. format_uint_with_hex(ssi, 24))
    end
    if cpti == 2 then
        local ext = cursor_read_uint(cur, 24)
        if ext == nil then
            add_text(tree, range, "Calling party extension: truncated")
            return
        end
        add_text(tree, range, "Calling party extension: " .. format_uint_with_hex(ext, 24))
    end
end

local function parse_cmce_party_address(cur, tree, range, label_prefix)
    local cpti = cursor_read_uint(cur, 2)
    if cpti == nil then
        add_text(tree, range, label_prefix .. ": truncated")
        return nil
    end
    add_named_value(tree, range, label_prefix, cpti, PARTY_TYPE_NAMES, 2)

    if cpti == 1 or cpti == 2 then
        local ssi = cursor_read_uint(cur, 24)
        if ssi == nil then
            add_text(tree, range, "Calling party SSI: truncated")
            return cpti
        end
        add_text(tree, range, "Calling party SSI: " .. format_uint_with_hex(ssi, 24))
    end
    if cpti == 2 then
        local ext = cursor_read_uint(cur, 24)
        if ext == nil then
            add_text(tree, range, "Calling party extension: truncated")
            return cpti
        end
        add_text(tree, range, "Calling party extension: " .. format_uint_with_hex(ext, 24))
    end
    return cpti
end

local function parse_mle_cmce_downlink(bits, tree, range)
    local cur = new_cursor(bits)
    local pdu_type = cursor_read_uint(cur, 5)
    if pdu_type == nil then
        add_text(tree, range, "CMCE PDU: truncated")
        return
    end

    local cmce_tree = tree:add(range, "CMCE: " .. lookup(CMCE_PDU_NAMES, pdu_type, "Unknown"))
    add_named_value(cmce_tree, range, "PDU type", pdu_type, CMCE_PDU_NAMES, 5)

    if pdu_type == 0 then
        local call_identifier = cursor_read_uint(cur, 14)
        local call_timeout_setup = cursor_read_uint(cur, 3)
        local reserved = cursor_read_uint(cur, 1)
        local simplex_duplex = cursor_read_uint(cur, 1)
        local call_queued = cursor_read_uint(cur, 1)
        if call_identifier == nil or call_timeout_setup == nil or reserved == nil or simplex_duplex == nil or call_queued == nil then
            add_text(cmce_tree, range, "D-ALERT: truncated")
            return
        end
        add_text(cmce_tree, range, "Call identifier: " .. format_uint_with_hex(call_identifier, 14))
        add_named_value(cmce_tree, range, "Call time-out set-up phase", call_timeout_setup, CALL_TIMEOUT_SETUP_NAMES, 3)
        add_text(cmce_tree, range, "Reserved: " .. bool_text(reserved))
        add_text(cmce_tree, range, "Simplex/duplex selection: " .. bool_text(simplex_duplex))
        add_text(cmce_tree, range, "Call queued: " .. bool_text(call_queued))

        local obit = read_obit(cur, cmce_tree, range, "D-ALERT")
        if obit == nil then
            return
        end
        if obit then
            parse_type2_struct(cur, cmce_tree, range, "Basic service information", parse_basic_service_information)
            parse_type2_uint(cur, cmce_tree, range, "Notification indicator", 6)
            parse_type3_generic(cur, cmce_tree, range, 3, "Facility", CMCE_TYPE3_NAMES)
            parse_type3_generic(cur, cmce_tree, range, 15, "Proprietary", CMCE_TYPE3_NAMES)
            finalize_optional_tail(cur, cmce_tree, range, "D-ALERT")
        end
    elseif pdu_type == 1 then
        local call_identifier = cursor_read_uint(cur, 14)
        local call_timeout_setup = cursor_read_uint(cur, 3)
        local hook_method_selection = cursor_read_uint(cur, 1)
        local simplex_duplex = cursor_read_uint(cur, 1)
        if call_identifier == nil or call_timeout_setup == nil or hook_method_selection == nil or simplex_duplex == nil then
            add_text(cmce_tree, range, "D-CALL PROCEEDING: truncated")
            return
        end
        add_text(cmce_tree, range, "Call identifier: " .. format_uint_with_hex(call_identifier, 14))
        add_named_value(cmce_tree, range, "Call time-out set-up phase", call_timeout_setup, CALL_TIMEOUT_SETUP_NAMES, 3)
        add_text(cmce_tree, range, "Hook method selection: " .. bool_text(hook_method_selection))
        add_text(cmce_tree, range, "Simplex/duplex selection: " .. bool_text(simplex_duplex))

        local obit = read_obit(cur, cmce_tree, range, "D-CALL PROCEEDING")
        if obit == nil then
            return
        end
        if obit then
            parse_type2_struct(cur, cmce_tree, range, "Basic service information", parse_basic_service_information)
            parse_type2_uint(cur, cmce_tree, range, "Call status", 3, CALL_STATUS_NAMES)
            parse_type2_uint(cur, cmce_tree, range, "Notification indicator", 6)
            parse_type3_generic(cur, cmce_tree, range, 3, "Facility", CMCE_TYPE3_NAMES)
            parse_type3_generic(cur, cmce_tree, range, 15, "Proprietary", CMCE_TYPE3_NAMES)
            finalize_optional_tail(cur, cmce_tree, range, "D-CALL PROCEEDING")
        end
    elseif pdu_type == 2 then
        local call_identifier = cursor_read_uint(cur, 14)
        local call_timeout = cursor_read_uint(cur, 4)
        local hook_method_selection = cursor_read_uint(cur, 1)
        local simplex_duplex = cursor_read_uint(cur, 1)
        local transmission_grant = cursor_read_uint(cur, 2)
        local transmission_request_permission = cursor_read_uint(cur, 1)
        local call_ownership = cursor_read_uint(cur, 1)
        if call_identifier == nil or call_timeout == nil or hook_method_selection == nil or simplex_duplex == nil or transmission_grant == nil or transmission_request_permission == nil or call_ownership == nil then
            add_text(cmce_tree, range, "D-CONNECT: truncated")
            return
        end
        add_text(cmce_tree, range, "Call identifier: " .. format_uint_with_hex(call_identifier, 14))
        add_named_value(cmce_tree, range, "Call time-out", call_timeout, CALL_TIMEOUT_NAMES, 4)
        add_text(cmce_tree, range, "Hook method selection: " .. bool_text(hook_method_selection))
        add_text(cmce_tree, range, "Simplex/duplex selection: " .. bool_text(simplex_duplex))
        add_named_value(cmce_tree, range, "Transmission grant", transmission_grant, TRANSMISSION_GRANT_NAMES, 2)
        add_text(cmce_tree, range, "Transmission request permission: " .. bool_text(transmission_request_permission))
        add_text(cmce_tree, range, "Call ownership: " .. bool_text(call_ownership))

        local obit = read_obit(cur, cmce_tree, range, "D-CONNECT")
        if obit == nil then
            return
        end
        if obit then
            parse_type2_uint(cur, cmce_tree, range, "Call priority", 4)
            parse_type2_struct(cur, cmce_tree, range, "Basic service information", parse_basic_service_information)
            parse_type2_uint(cur, cmce_tree, range, "Temporary address", 24)
            parse_type2_uint(cur, cmce_tree, range, "Notification indicator", 6)
            parse_type3_generic(cur, cmce_tree, range, 3, "Facility", CMCE_TYPE3_NAMES)
            parse_type3_generic(cur, cmce_tree, range, 15, "Proprietary", CMCE_TYPE3_NAMES)
            finalize_optional_tail(cur, cmce_tree, range, "D-CONNECT")
        end
    elseif pdu_type == 3 then
        local call_identifier = cursor_read_uint(cur, 14)
        local call_timeout = cursor_read_uint(cur, 4)
        local transmission_grant = cursor_read_uint(cur, 2)
        local transmission_request_permission = cursor_read_uint(cur, 1)
        if call_identifier == nil or call_timeout == nil or transmission_grant == nil or transmission_request_permission == nil then
            add_text(cmce_tree, range, "D-CONNECT ACKNOWLEDGE: truncated")
            return
        end
        add_text(cmce_tree, range, "Call identifier: " .. format_uint_with_hex(call_identifier, 14))
        add_named_value(cmce_tree, range, "Call time-out", call_timeout, CALL_TIMEOUT_NAMES, 4)
        add_named_value(cmce_tree, range, "Transmission grant", transmission_grant, TRANSMISSION_GRANT_NAMES, 2)
        add_text(cmce_tree, range, "Transmission request permission: " .. bool_text(transmission_request_permission))

        local obit = read_obit(cur, cmce_tree, range, "D-CONNECT ACKNOWLEDGE")
        if obit == nil then
            return
        end
        if obit then
            parse_type2_uint(cur, cmce_tree, range, "Notification indicator", 6)
            parse_type3_generic(cur, cmce_tree, range, 3, "Facility", CMCE_TYPE3_NAMES)
            parse_type3_generic(cur, cmce_tree, range, 15, "Proprietary", CMCE_TYPE3_NAMES)
            finalize_optional_tail(cur, cmce_tree, range, "D-CONNECT ACKNOWLEDGE")
        end
    elseif pdu_type == 4 or pdu_type == 6 then
        local call_identifier = cursor_read_uint(cur, 14)
        local disconnect_cause = cursor_read_uint(cur, 5)
        if call_identifier == nil or disconnect_cause == nil then
            add_text(cmce_tree, range, (pdu_type == 4 and "D-DISCONNECT" or "D-RELEASE") .. ": truncated")
            return
        end
        add_text(cmce_tree, range, "Call identifier: " .. format_uint_with_hex(call_identifier, 14))
        add_named_value(cmce_tree, range, "Disconnect cause", disconnect_cause, DISCONNECT_CAUSE_NAMES, 5)

        local obit = read_obit(cur, cmce_tree, range, pdu_type == 4 and "D-DISCONNECT" or "D-RELEASE")
        if obit == nil then
            return
        end
        if obit then
            parse_type2_uint(cur, cmce_tree, range, "Notification indicator", 6)
            parse_type3_generic(cur, cmce_tree, range, 3, "Facility", CMCE_TYPE3_NAMES)
            parse_type3_generic(cur, cmce_tree, range, 15, "Proprietary", CMCE_TYPE3_NAMES)
            finalize_optional_tail(cur, cmce_tree, range, pdu_type == 4 and "D-DISCONNECT" or "D-RELEASE")
        end
    elseif pdu_type == 7 then
        local call_identifier = cursor_read_uint(cur, 14)
        local call_timeout = cursor_read_uint(cur, 4)
        local hook_method_selection = cursor_read_uint(cur, 1)
        local simplex_duplex = cursor_read_uint(cur, 1)
        local basic_service_tree = cmce_tree:add(range, "Basic service information")
        if call_identifier == nil or call_timeout == nil or hook_method_selection == nil or simplex_duplex == nil then
            add_text(cmce_tree, range, "D-SETUP: truncated")
            return
        end
        add_text(cmce_tree, range, "Call identifier: " .. format_uint_with_hex(call_identifier, 14))
        add_named_value(cmce_tree, range, "Call time-out", call_timeout, CALL_TIMEOUT_NAMES, 4)
        add_text(cmce_tree, range, "Hook method selection: " .. bool_text(hook_method_selection))
        add_text(cmce_tree, range, "Simplex/duplex selection: " .. bool_text(simplex_duplex))
        parse_basic_service_information(cur, basic_service_tree, range)

        local transmission_grant = cursor_read_uint(cur, 2)
        local transmission_request_permission = cursor_read_uint(cur, 1)
        local call_priority = cursor_read_uint(cur, 4)
        if transmission_grant == nil or transmission_request_permission == nil or call_priority == nil then
            add_text(cmce_tree, range, "D-SETUP fixed fields: truncated")
            return
        end
        add_named_value(cmce_tree, range, "Transmission grant", transmission_grant, TRANSMISSION_GRANT_NAMES, 2)
        add_text(cmce_tree, range, "Transmission request permission: " .. bool_text(transmission_request_permission))
        add_text(cmce_tree, range, "Call priority: " .. call_priority)

        local obit = read_obit(cur, cmce_tree, range, "D-SETUP")
        if obit == nil then
            return
        end
        if obit then
            parse_type2_uint(cur, cmce_tree, range, "Notification indicator", 6)
            parse_type2_uint(cur, cmce_tree, range, "Temporary address", 24)
            parse_cmce_setup_optional_calling_party(cur, cmce_tree, range)
            parse_type3_generic(cur, cmce_tree, range, 2, "External subscriber number", CMCE_TYPE3_NAMES)
            parse_type3_generic(cur, cmce_tree, range, 3, "Facility", CMCE_TYPE3_NAMES)
            parse_type3_generic(cur, cmce_tree, range, 6, "DM-MS address", CMCE_TYPE3_NAMES)
            parse_type3_generic(cur, cmce_tree, range, 15, "Proprietary", CMCE_TYPE3_NAMES)
            finalize_optional_tail(cur, cmce_tree, range, "D-SETUP")
        end
    elseif pdu_type == 8 then
        parse_cmce_party_address(cur, cmce_tree, range, "Calling party type identifier")
        local pre_coded_status = cursor_read_uint(cur, 16)
        if pre_coded_status == nil then
            add_text(cmce_tree, range, "Pre-coded status: truncated")
            return
        end
        add_text(cmce_tree, range, "Pre-coded status: " .. describe_pre_coded_status(pre_coded_status) .. " [" .. format_uint_with_hex(pre_coded_status, 16) .. "]")

        local obit = read_obit(cur, cmce_tree, range, "D-STATUS")
        if obit == nil then
            return
        end
        if obit then
            parse_type3_generic(cur, cmce_tree, range, 2, "External subscriber number", CMCE_TYPE3_NAMES)
            parse_type3_generic(cur, cmce_tree, range, 6, "DM-MS address", CMCE_TYPE3_NAMES)
            finalize_optional_tail(cur, cmce_tree, range, "D-STATUS")
        end
    elseif pdu_type == 15 then
        parse_cmce_party_address(cur, cmce_tree, range, "Calling party type identifier")
        local short_data_type = cursor_read_uint(cur, 2)
        if short_data_type == nil then
            add_text(cmce_tree, range, "Short data type identifier: truncated")
            return
        end
        add_named_value(cmce_tree, range, "Short data type identifier", short_data_type, SHORT_DATA_TYPE_NAMES, 2)

        local sds_tree = cmce_tree:add(range, "SDS user data")
        if short_data_type == 0 then
            local bits = cursor_read_bits(cur, 16)
            if bits == nil then
                add_text(sds_tree, range, "User defined data-1: truncated")
                return
            end
            parse_sds_payload(bits, sds_tree, range)
        elseif short_data_type == 1 then
            local bits = cursor_read_bits(cur, 32)
            if bits == nil then
                add_text(sds_tree, range, "User defined data-2: truncated")
                return
            end
            parse_sds_payload(bits, sds_tree, range)
        elseif short_data_type == 2 then
            local bits = cursor_read_bits(cur, 64)
            if bits == nil then
                add_text(sds_tree, range, "User defined data-3: truncated")
                return
            end
            parse_sds_payload(bits, sds_tree, range)
        else
            local length_indicator = cursor_read_uint(cur, 11)
            if length_indicator == nil then
                add_text(sds_tree, range, "Length indicator: truncated")
                return
            end
            add_text(sds_tree, range, "Length indicator: " .. length_indicator .. " bits")
            local bits = cursor_read_bits(cur, length_indicator)
            if bits == nil then
                add_text(sds_tree, range, "User defined data-4: truncated")
                return
            end
            parse_sds_payload(bits, sds_tree, range)
        end

        local obit = read_obit(cur, cmce_tree, range, "D-SDS-DATA")
        if obit == nil then
            return
        end
        if obit then
            parse_type3_generic(cur, cmce_tree, range, 2, "External subscriber number", CMCE_TYPE3_NAMES)
            parse_type3_generic(cur, cmce_tree, range, 6, "DM-MS address", CMCE_TYPE3_NAMES)
            finalize_optional_tail(cur, cmce_tree, range, "D-SDS-DATA")
        end
    else
        parse_type34_tail_only(cur, cmce_tree, range, "CMCE payload")
    end
end

local function parse_mle_cmce_uplink(bits, tree, range)
    local cur = new_cursor(bits)
    local pdu_type = cursor_read_uint(cur, 5)
    if pdu_type == nil then
        add_text(tree, range, "CMCE PDU: truncated")
        return
    end

    local cmce_tree = tree:add(range, "CMCE: " .. lookup(CMCE_PDU_NAMES_UL, pdu_type, "Unknown"))
    add_named_value(cmce_tree, range, "PDU type", pdu_type, CMCE_PDU_NAMES_UL, 5)

    if pdu_type == 8 then
        parse_cmce_party_address(cur, cmce_tree, range, "Calling party type identifier")
        local pre_coded_status = cursor_read_uint(cur, 16)
        if pre_coded_status == nil then
            add_text(cmce_tree, range, "Pre-coded status: truncated")
            return
        end
        add_text(cmce_tree, range, "Pre-coded status: " .. describe_pre_coded_status(pre_coded_status) .. " [" .. format_uint_with_hex(pre_coded_status, 16) .. "]")
        if cursor_remaining(cur) > 0 then
            local tail = cursor_read_bits(cur, cursor_remaining(cur))
            add_text(cmce_tree, range, string.format("U-STATUS trailing bits (%u): %s", #tail, preview_bits(tail, 160)))
        end
    elseif pdu_type == 15 then
        parse_cmce_party_address(cur, cmce_tree, range, "Calling party type identifier")
        local short_data_type = cursor_read_uint(cur, 2)
        if short_data_type == nil then
            add_text(cmce_tree, range, "Short data type identifier: truncated")
            return
        end
        add_named_value(cmce_tree, range, "Short data type identifier", short_data_type, SHORT_DATA_TYPE_NAMES, 2)

        local sds_tree = cmce_tree:add(range, "SDS user data")
        if short_data_type == 0 then
            local payload = cursor_read_bits(cur, 16)
            if payload == nil then
                add_text(sds_tree, range, "User defined data-1: truncated")
                return
            end
            parse_sds_payload(payload, sds_tree, range)
        elseif short_data_type == 1 then
            local payload = cursor_read_bits(cur, 32)
            if payload == nil then
                add_text(sds_tree, range, "User defined data-2: truncated")
                return
            end
            parse_sds_payload(payload, sds_tree, range)
        elseif short_data_type == 2 then
            local payload = cursor_read_bits(cur, 64)
            if payload == nil then
                add_text(sds_tree, range, "User defined data-3: truncated")
                return
            end
            parse_sds_payload(payload, sds_tree, range)
        else
            local length_indicator = cursor_read_uint(cur, 11)
            if length_indicator == nil then
                add_text(sds_tree, range, "Length indicator: truncated")
                return
            end
            add_text(sds_tree, range, "Length indicator: " .. length_indicator .. " bits")
            local payload = cursor_read_bits(cur, length_indicator)
            if payload == nil then
                add_text(sds_tree, range, "User defined data-4: truncated")
                return
            end
            parse_sds_payload(payload, sds_tree, range)
        end

        if cursor_remaining(cur) > 0 then
            local tail = cursor_read_bits(cur, cursor_remaining(cur))
            add_text(cmce_tree, range, string.format("U-SDS-DATA trailing bits (%u): %s", #tail, preview_bits(tail, 160)))
        end
    else
        parse_type34_tail_only(cur, cmce_tree, range, "CMCE payload")
    end
end

local function parse_mle_downlink(bits, tree, range)
    local cur = new_cursor(bits)
    local pdu_type = cursor_read_uint(cur, 3)
    if pdu_type == nil then
        add_text(tree, range, "MLE downlink: truncated")
        return
    end

    local mle_tree = tree:add(range, "MLE downlink")
    add_named_value(mle_tree, range, "PDU type", pdu_type, MLE_PDU_NAMES_DL, 3)

    if pdu_type == 2 then
        local cell_reselect_parameters = cursor_read_uint(cur, 16)
        local cell_load_ca = cursor_read_uint(cur, 2)
        if cell_reselect_parameters == nil or cell_load_ca == nil then
            add_text(mle_tree, range, "D-NWRK-BROADCAST body: truncated")
            return
        end

        local nwrk_tree = mle_tree:add(range, "MLE: D-NWRK-BROADCAST")
        add_text(nwrk_tree, range, "Cell re-select parameters: " .. format_uint_with_hex(cell_reselect_parameters, 16))
        add_text(nwrk_tree, range, "Cell load CA: " .. format_uint_with_hex(cell_load_ca, 2))

        local obit = read_obit(cur, nwrk_tree, range, "D-NWRK-BROADCAST")
        if obit == nil then
            return
        end
        add_text(nwrk_tree, range, "Optional fields present: " .. tostring(obit))
        if not obit then
            if cursor_remaining(cur) > 0 then
                local tail = cursor_read_bits(cur, cursor_remaining(cur)) or ""
                if tail ~= "" then
                    add_text(nwrk_tree, range, string.format("D-NWRK-BROADCAST trailing bits (%u): %s", #tail, preview_bits(tail, 160)))
                end
            end
            return
        end

        local tetra_network_time = parse_type2_uint(cur, nwrk_tree, range, "TETRA network time", 48)
        local number_of_ca_neighbour_cells = parse_type2_uint(cur, nwrk_tree, range, "Number of CA neighbour cells", 3)

        if number_of_ca_neighbour_cells ~= nil and number_of_ca_neighbour_cells > 0 then
            add_text(nwrk_tree, range, "Neighbour cell information for CA: not decoded")
        end

        if cursor_remaining(cur) > 0 then
            local tail = cursor_read_bits(cur, cursor_remaining(cur)) or ""
            if tail ~= "" then
                add_text(nwrk_tree, range, string.format("D-NWRK-BROADCAST trailing bits (%u): %s", #tail, preview_bits(tail, 160)))
            end
        end
        return
    end

    local payload = cursor_read_bits(cur, cursor_remaining(cur)) or ""
    if payload ~= "" then
        add_text(mle_tree, range, string.format("%s payload (%u bits): %s", lookup(MLE_PDU_NAMES_DL, pdu_type, "MLE"), #payload, preview_bits(payload, 160)))
    end
end

local function parse_tl_sdu(bits, tree, range, direction)
    if bits == nil or bits == "" then
        add_text(tree, range, "TL-SDU: <empty>")
        return
    end

    local cur = new_cursor(bits)
    local protocol = cursor_read_uint(cur, 3)
    if protocol == nil then
        add_text(tree, range, "TL-SDU: truncated")
        return
    end

    local tl_tree = tree:add(range, "TL-SDU")
    add_named_value(tl_tree, range, "MLE protocol discriminator", protocol, MLE_PROTOCOL_NAMES, 3)

    local payload_bits = cursor_read_bits(cur, cursor_remaining(cur)) or ""
    if direction == 0 then
        if protocol == 1 then
            parse_mle_mm_downlink(payload_bits, tl_tree, range)
        elseif protocol == 2 then
            parse_mle_cmce_downlink(payload_bits, tl_tree, range)
        elseif protocol == 5 then
            parse_mle_downlink(payload_bits, tl_tree, range)
        else
            add_text(tl_tree, range, string.format("%s payload (%u bits): %s", lookup(MLE_PROTOCOL_NAMES, protocol, "Unknown"), #payload_bits, preview_bits(payload_bits, 160)))
        end
    else
        if protocol == 1 then
            parse_mle_mm_uplink(payload_bits, tl_tree, range)
        elseif protocol == 2 then
            parse_mle_cmce_uplink(payload_bits, tl_tree, range)
        else
            add_text(tl_tree, range, string.format("Uplink %s payload (%u bits): %s", lookup(MLE_PROTOCOL_NAMES, protocol, "Unknown"), #payload_bits, preview_bits(payload_bits, 160)))
        end
    end
end

local function parse_llc(bits, tree, range, direction)
    if bits == nil or bits == "" then
        add_text(tree, range, "TM-SDU: <empty>")
        return
    end

    local cur = new_cursor(bits)
    local pdu_type = cursor_read_uint(cur, 4)
    if pdu_type == nil then
        add_text(tree, range, "LLC: truncated")
        return
    end

    local llc_tree = tree:add(range, "LLC")
    add_named_value(llc_tree, range, "PDU type", pdu_type, LLC_PDU_NAMES, 4)

    if pdu_type >= 8 then
        local tail = cursor_read_bits(cur, cursor_remaining(cur)) or ""
        if tail ~= "" then
            add_text(llc_tree, range, string.format("Non-basic LLC raw tail (%u bits): %s", #tail, preview_bits(tail, 160)))
        else
            add_text(llc_tree, range, "Non-basic LLC with no payload")
        end
        return
    end

    local has_fcs = pdu_type >= 4 and pdu_type <= 7
    local basic_type = pdu_type % 4

    if basic_type == 0 then
        local nr = cursor_read_uint(cur, 1)
        local ns = cursor_read_uint(cur, 1)
        if nr == nil or ns == nil then
            add_text(llc_tree, range, "BL-ADATA header: truncated")
            return
        end
        add_text(llc_tree, range, "N(R): " .. nr)
        add_text(llc_tree, range, "N(S): " .. ns)
    elseif basic_type == 1 then
        local ns = cursor_read_uint(cur, 1)
        if ns == nil then
            add_text(llc_tree, range, "BL-DATA header: truncated")
            return
        end
        add_text(llc_tree, range, "N(S): " .. ns)
    elseif basic_type == 3 then
        local nr = cursor_read_uint(cur, 1)
        if nr == nil then
            add_text(llc_tree, range, "BL-ACK header: truncated")
            return
        end
        add_text(llc_tree, range, "N(R): " .. nr)
    end

    local remaining = cursor_remaining(cur)
    if remaining < 0 then
        add_text(llc_tree, range, "LLC: invalid remaining length")
        return
    end

    local payload_bits = nil
    local fcs_bits = nil
    if has_fcs == 1 then
        if remaining < 32 then
            add_text(llc_tree, range, "FCS flagged but fewer than 32 bits remain")
            payload_bits = cursor_read_bits(cur, remaining)
        else
            payload_bits = cursor_read_bits(cur, remaining - 32)
            fcs_bits = cursor_read_bits(cur, 32)
        end
    else
        payload_bits = cursor_read_bits(cur, remaining)
    end

    if fcs_bits ~= nil then
        add_text(llc_tree, range, "FCS: 0x" .. bit_string_to_hex(fcs_bits))
    end

    if basic_type == 3 then
        if payload_bits ~= nil and payload_bits ~= "" then
            add_text(llc_tree, range, "BL-ACK trailing bits: " .. preview_bits(payload_bits, 160))
        end
        return
    end

    parse_tl_sdu(payload_bits or "", llc_tree, range, direction)
end

local function parse_access_assign(bits, tree, range, is_frame18)
    local header = bits_to_uint(bits, 0, 2)
    local field1 = bits_to_uint(bits, 2, 6)
    local field2 = bits_to_uint(bits, 8, 6)
    if header == nil or field1 == nil or field2 == nil then
        add_text(tree, range, "ACCESS-ASSIGN truncated")
        return
    end

    local aa_tree = tree:add(range, string.format("ACCESS-ASSIGN%s", is_frame18 and " FR18" or ""))
    add_text(aa_tree, range, string.format("Header: %u", header))

    if is_frame18 then
        if header <= 2 then
            local ul_usage = ({ [0] = "CommonOnly", [1] = "CommonAndAssigned", [2] = "AssignedOnly" })[header]
            add_text(aa_tree, range, "UL usage: " .. ul_usage)
            add_text(aa_tree, range, string.format("Access field 1: code=%u base_frame_len=%u", math.floor(field1 / 16), field1 % 16))
            add_text(aa_tree, range, string.format("Access field 2: code=%u base_frame_len=%u", math.floor(field2 / 16), field2 % 16))
        else
            local traffic = field1 >= 4 and ("Traffic(" .. field1 .. ")") or ("Invalid(" .. field1 .. ")")
            add_text(aa_tree, range, "UL traffic usage: " .. traffic)
            add_text(aa_tree, range, string.format("Shared access field: code=%u base_frame_len=%u", math.floor(field2 / 16), field2 % 16))
        end
        return
    end

    if header == 0 then
        add_text(aa_tree, range, "DL usage: CommonControl")
        add_text(aa_tree, range, "UL usage: CommonOnly")
        add_text(aa_tree, range, string.format("Access field 1: code=%u base_frame_len=%u", math.floor(field1 / 16), field1 % 16))
        add_text(aa_tree, range, string.format("Access field 2: code=%u base_frame_len=%u", math.floor(field2 / 16), field2 % 16))
    elseif header == 1 then
        local dl_usage = ({ [0] = "Unallocated", [1] = "AssignedControl", [2] = "CommonControl", [3] = "CommonAndAssigned" })[field1]
        if dl_usage == nil then
            dl_usage = "Traffic(" .. field1 .. ")"
        end
        add_text(aa_tree, range, "DL usage: " .. dl_usage)
        add_text(aa_tree, range, "UL usage: CommonAndAssigned")
        add_text(aa_tree, range, string.format("Shared access field: code=%u base_frame_len=%u", math.floor(field2 / 16), field2 % 16))
    elseif header == 2 then
        local dl_usage = ({ [0] = "Unallocated", [1] = "AssignedControl", [2] = "CommonControl", [3] = "CommonAndAssigned" })[field1]
        if dl_usage == nil then
            dl_usage = "Traffic(" .. field1 .. ")"
        end
        add_text(aa_tree, range, "DL usage: " .. dl_usage)
        add_text(aa_tree, range, "UL usage: AssignedOnly")
        add_text(aa_tree, range, string.format("Shared access field: code=%u base_frame_len=%u", math.floor(field2 / 16), field2 % 16))
    else
        local dl_usage = ({ [0] = "Unallocated", [1] = "AssignedControl", [2] = "CommonControl", [3] = "CommonAndAssigned" })[field1]
        if dl_usage == nil then
            dl_usage = "Traffic(" .. field1 .. ")"
        end
        local ul_usage = field2 == 0 and "Unallocated" or (field2 >= 4 and ("Traffic(" .. field2 .. ")") or ("Invalid(" .. field2 .. ")"))
        add_text(aa_tree, range, "DL usage: " .. dl_usage)
        add_text(aa_tree, range, "UL usage: " .. ul_usage)
    end
end

local function parse_bsch(bits, tree, range)
    local sync_tree = tree:add(range, "BSCH: MAC-SYNC + D-MLE-SYNC")
    local system_code = bits_to_uint(bits, 0, 4)
    local colour_code = bits_to_uint(bits, 4, 6)
    local timeslot = bits_to_uint(bits, 10, 2)
    local frame = bits_to_uint(bits, 12, 5)
    local multiframe = bits_to_uint(bits, 17, 6)
    local sharing_mode = bits_to_uint(bits, 23, 2)
    local ts_reserved = bits_to_uint(bits, 25, 3)
    local u_plane_dtx = bits_to_uint(bits, 28, 1)
    local frame18_ext = bits_to_uint(bits, 29, 1)
    local reserved = bits_to_uint(bits, 30, 1)

    if system_code == nil or colour_code == nil then
        add_text(sync_tree, range, "MAC-SYNC truncated")
        return
    end

    add_text(sync_tree, range, string.format("MAC-SYNC: system_code=%u colour_code=%u time=%u/%u/%u sharing_mode=%u ts_reserved_frames=%u u_plane_dtx=%s frame_18_ext=%s reserved=%u",
        system_code,
        colour_code,
        (multiframe or 0),
        (frame or 0),
        ((timeslot or 0) + 1),
        sharing_mode or 0,
        ts_reserved or 0,
        u_plane_dtx == 1 and "true" or "false",
        frame18_ext == 1 and "true" or "false",
        reserved or 0
    ))

    local mcc = bits_to_uint(bits, 31, 10)
    local mnc = bits_to_uint(bits, 41, 14)
    local neighbor_cell_broadcast = bits_to_uint(bits, 55, 2)
    local cell_load_ca = bits_to_uint(bits, 57, 2)
    local late_entry_supported = bits_to_uint(bits, 59, 1)
    if mcc ~= nil then
        add_text(sync_tree, range, string.format("D-MLE-SYNC: mcc=%u mnc=%u neighbor_cell_broadcast=%u cell_load_ca=%u late_entry_supported=%s",
            mcc,
            mnc or 0,
            neighbor_cell_broadcast or 0,
            cell_load_ca or 0,
            late_entry_supported == 1 and "true" or "false"
        ))
    end
end

local function parse_bs_service_details(bits, start_bit, tree, range)
    local flags = {
        { name = "registration", enabled = bits_to_uint(bits, start_bit + 0, 1) == 1 },
        { name = "deregistration", enabled = bits_to_uint(bits, start_bit + 1, 1) == 1 },
        { name = "priority_cell", enabled = bits_to_uint(bits, start_bit + 2, 1) == 1 },
        { name = "no_minimum_mode", enabled = bits_to_uint(bits, start_bit + 3, 1) == 1 },
        { name = "migration", enabled = bits_to_uint(bits, start_bit + 4, 1) == 1 },
        { name = "system_wide_services", enabled = bits_to_uint(bits, start_bit + 5, 1) == 1 },
        { name = "voice_service", enabled = bits_to_uint(bits, start_bit + 6, 1) == 1 },
        { name = "circuit_mode_data_service", enabled = bits_to_uint(bits, start_bit + 7, 1) == 1 },
        { name = "sndcp_service", enabled = bits_to_uint(bits, start_bit + 9, 1) == 1 },
        { name = "aie_service", enabled = bits_to_uint(bits, start_bit + 10, 1) == 1 },
        { name = "advanced_link", enabled = bits_to_uint(bits, start_bit + 11, 1) == 1 },
    }
    add_flag_summary(tree, range, "BS service details", flags)
end

local function parse_sysinfo_optional(bits, option_field, tree, range)
    if option_field == 0 or option_field == 1 then
        local common_frames = bits_slice(bits, 62, 20)
        if common_frames ~= nil then
            add_text(tree, range, string.format("%s: %s",
                lookup(SYSINFO_OPT_NAMES, option_field, "Unknown"),
                common_frames
            ))
        end
    elseif option_field == 2 then
        local imm = bits_to_uint(bits, 62, 4)
        local wt = bits_to_uint(bits, 66, 4)
        local nu = bits_to_uint(bits, 70, 4)
        local fl_factor = bits_to_uint(bits, 74, 1)
        local ts_ptr = bits_to_uint(bits, 75, 4)
        local min_pdu_prio = bits_to_uint(bits, 79, 3)
        add_text(tree, range, string.format(
            "DefaultDefForAccCodeA: imm=%u wt=%u nu=%u fl_factor=%s ts_ptr=%u min_pdu_prio=%u",
            imm or 0,
            wt or 0,
            nu or 0,
            fl_factor == 1 and "true" or "false",
            ts_ptr or 0,
            min_pdu_prio or 0
        ))
    elseif option_field == 3 then
        local auth_required = bits_to_uint(bits, 62, 1)
        local class1_supported = bits_to_uint(bits, 63, 1)
        local class3_marker = bits_to_uint(bits, 64, 1)
        local security_tail = bits_slice(bits, 65, 5) or ""
        local sdstl_addressing_method = bits_to_uint(bits, 70, 2)
        local gck_supported = bits_to_uint(bits, 72, 1)
        local section = bits_to_uint(bits, 73, 2)
        local section_data = bits_to_uint(bits, 75, 7)
        add_text(tree, range, string.format(
            "ExtServicesBroadcast: auth_required=%s class1_supported=%s class3_marker=%u security_tail=%s sdstl_addressing_method=%u gck_supported=%s section=%u section_data=%u",
            auth_required == 1 and "true" or "false",
            class1_supported == 1 and "true" or "false",
            class3_marker or 0,
            security_tail,
            sdstl_addressing_method or 0,
            gck_supported == 1 and "true" or "false",
            section or 0,
            section_data or 0
        ))
    end
end

local function parse_bnch(bits, tree, range)
    local bnch_tree = tree:add(range, "BNCH: MAC-SYSINFO + D-MLE-SYSINFO")
    local pdu_type = bits_to_uint(bits, 0, 2)
    local pdu_subtype = bits_to_uint(bits, 2, 2)
    if pdu_type ~= 2 or pdu_subtype ~= 0 then
        add_text(bnch_tree, range, string.format("Unexpected MAC-SYSINFO header: pdu_type=%s subtype=%s", tostring(pdu_type), tostring(pdu_subtype)))
        return
    end

    local main_carrier = bits_to_uint(bits, 4, 12)
    local freq_band = bits_to_uint(bits, 16, 4)
    local freq_offset_index = bits_to_uint(bits, 20, 2)
    local duplex_spacing = bits_to_uint(bits, 22, 3)
    local reverse_operation = bits_to_uint(bits, 25, 1)
    local num_of_csch = bits_to_uint(bits, 26, 2)
    local ms_txpwr_max_cell = bits_to_uint(bits, 28, 3)
    local rxlev_access_min = bits_to_uint(bits, 31, 4)
    local access_parameter = bits_to_uint(bits, 35, 4)
    local radio_dl_timeout = bits_to_uint(bits, 39, 4)
    local has_cck_field = bits_to_uint(bits, 43, 1)
    local cck_or_hfn = bits_to_uint(bits, 44, 16)
    local option_field = bits_to_uint(bits, 60, 2)
    add_text(bnch_tree, range, string.format(
        "MAC-SYSINFO: main_carrier=%u freq_band=%u freq_offset_index=%u duplex_spacing=%u reverse_operation=%s num_of_csch=%u ms_txpwr_max_cell=%u rxlev_access_min=%u access_parameter=%u radio_dl_timeout=%u %s=%u option_field=%s",
        main_carrier or 0,
        freq_band or 0,
        freq_offset_index or 0,
        duplex_spacing or 0,
        reverse_operation == 1 and "true" or "false",
        num_of_csch or 0,
        ms_txpwr_max_cell or 0,
        rxlev_access_min or 0,
        access_parameter or 0,
        radio_dl_timeout or 0,
        has_cck_field == 1 and "cck_id" or "hyperframe_number",
        cck_or_hfn or 0,
        lookup(SYSINFO_OPT_NAMES, option_field, "Unknown")
    ))

    if option_field ~= nil then
        parse_sysinfo_optional(bits, option_field, bnch_tree, range)
    end

    local location_area = bits_to_uint(bits, 82, 14)
    local subscriber_class = bits_to_uint(bits, 96, 16)
    if location_area ~= nil and subscriber_class ~= nil then
        add_text(bnch_tree, range, string.format("D-MLE-SYSINFO: location_area=%u subscriber_class=0x%04X", location_area, subscriber_class))
        parse_bs_service_details(bits, 112, bnch_tree, range)
    end
end

local function parse_tm_sdu(bits, tree, range, direction, label)
    if bits == nil or bits == "" then
        return
    end
    local tm_tree = tree:add(range, string.format("%s (%u bits)", label, #bits))
    add_text(tm_tree, range, "Raw bits: " .. preview_bits(bits, 160))
    parse_llc(bits, tm_tree, range, direction)
end

local function count_fill_bits(bits, pdu_len_bits, payload_pos)
    if bits == nil or pdu_len_bits == nil or payload_pos == nil then
        return 0
    end
    if pdu_len_bits <= payload_pos or pdu_len_bits > #bits then
        return 0
    end

    for idx = pdu_len_bits, payload_pos + 1, -1 do
        local c = bits:byte(idx)
        if c == nil then
            return 0
        end
        if c == 49 then
            return pdu_len_bits - idx + 1
        end
    end

    return 0
end

local function extract_mac_payload(bits, payload_pos, pdu_len_bits, has_fill_bits)
    if bits == nil or payload_pos == nil then
        return nil
    end

    local effective_pdu_len = pdu_len_bits or #bits
    if effective_pdu_len > #bits then
        effective_pdu_len = #bits
    end
    if effective_pdu_len <= payload_pos then
        return ""
    end

    local payload_end = effective_pdu_len
    if has_fill_bits == 1 then
        local num_fill_bits = count_fill_bits(bits, effective_pdu_len, payload_pos)
        if num_fill_bits > 0 and num_fill_bits <= (payload_end - payload_pos) then
            payload_end = payload_end - num_fill_bits
        end
    end

    if payload_end <= payload_pos then
        return ""
    end

    return bits_slice(bits, payload_pos, payload_end - payload_pos)
end

local function sch_hu_fragment_key(ctx)
    if ctx == nil or ctx.direction ~= 1 or ctx.logical_channel ~= 7 or ctx.timeslot == nil then
        return nil
    end
    return tostring(ctx.timeslot)
end

local function remember_sch_hu_fragment_in(cache, ctx, fragment_bits, addr_text)
    local key = sch_hu_fragment_key(ctx)
    if key == nil or fragment_bits == nil or fragment_bits == "" then
        return
    end
    cache[key] = {
        bits = fragment_bits,
        addr = addr_text,
        packet_number = ctx.packet_number or 0,
        consumed_by = nil,
    }
end

local function remember_sch_hu_fragment(ctx, fragment_bits, addr_text)
    remember_sch_hu_fragment_in(pending_sch_hu_fragments, ctx, fragment_bits, addr_text)
end

local function remember_sch_hu_summary_fragment(ctx, fragment_bits, addr_text)
    remember_sch_hu_fragment_in(pending_sch_hu_summary_fragments, ctx, fragment_bits, addr_text)
end

local function get_sch_hu_fragment_from(cache, ctx)
    local key = sch_hu_fragment_key(ctx)
    if key == nil then
        return nil
    end
    local pending = cache[key]
    if pending == nil then
        return nil
    end
    local current_packet = ctx.packet_number or 0
    if pending.packet_number ~= nil and current_packet > 0 then
        local delta = current_packet - pending.packet_number
        if delta <= 0 or delta > 32 then
            cache[key] = nil
            return nil
        end
    end
    if pending.consumed_by ~= nil and pending.consumed_by ~= current_packet then
        return nil
    end
    return pending
end

local function get_sch_hu_fragment(ctx)
    return get_sch_hu_fragment_from(pending_sch_hu_fragments, ctx)
end

local function get_sch_hu_summary_fragment(ctx)
    return get_sch_hu_fragment_from(pending_sch_hu_summary_fragments, ctx)
end

local function pop_sch_hu_fragment_from(cache, ctx)
    local pending = get_sch_hu_fragment_from(cache, ctx)
    if pending ~= nil then
        pending.consumed_by = ctx.packet_number or 0
    end
    return pending
end

local function pop_sch_hu_fragment(ctx)
    return pop_sch_hu_fragment_from(pending_sch_hu_fragments, ctx)
end

local function parse_mac_resource(bits, tree, range, direction)
    local fill_bits = bits_to_uint(bits, 2, 1)
    local pos_of_grant = bits_to_uint(bits, 3, 1)
    local encryption_mode = bits_to_uint(bits, 4, 2)
    local random_access_flag = bits_to_uint(bits, 6, 1)
    local length_ind = bits_to_uint(bits, 7, 6)
    local addr_type = bits_to_uint(bits, 13, 3)
    if fill_bits == nil or addr_type == nil then
        add_text(tree, range, "MAC-RESOURCE truncated")
        return
    end

    local mr_tree = tree:add(range, "MAC-RESOURCE")
    add_text(mr_tree, range, string.format("fill_bits=%s pos_of_grant=%u encryption_mode=%u random_access_flag=%s length_ind=%u addr_type=%s",
        fill_bits == 1 and "true" or "false",
        pos_of_grant or 0,
        encryption_mode or 0,
        random_access_flag == 1 and "true" or "false",
        length_ind or 0,
        lookup(RESOURCE_ADDR_NAMES, addr_type, "Unknown")
    ))

    local pos = 16
    if addr_type == 1 or addr_type == 3 or addr_type == 4 then
        local addr = bits_to_uint(bits, pos, 24)
        add_text(mr_tree, range, string.format("Address: %u", addr or 0))
        pos = pos + 24
    elseif addr_type == 2 then
        local event_label = bits_to_uint(bits, pos, 10)
        add_text(mr_tree, range, string.format("Event label: %u", event_label or 0))
        pos = pos + 10
    elseif addr_type == 5 or addr_type == 7 then
        local addr = bits_to_uint(bits, pos, 24)
        local event_label = bits_to_uint(bits, pos + 24, 10)
        add_text(mr_tree, range, string.format("Address: %u event_label=%u", addr or 0, event_label or 0))
        pos = pos + 34
    elseif addr_type == 6 then
        local addr = bits_to_uint(bits, pos, 24)
        local usage_marker = bits_to_uint(bits, pos + 24, 6)
        add_text(mr_tree, range, string.format("Address: %u usage_marker=%u", addr or 0, usage_marker or 0))
        pos = pos + 30
    else
        add_text(mr_tree, range, "Null PDU")
        return
    end

    local power_control_flag = bits_to_uint(bits, pos, 1)
    pos = pos + 1
    if power_control_flag == 1 then
        local power_control_element = bits_to_uint(bits, pos, 4)
        add_text(mr_tree, range, string.format("Power control: %u", power_control_element or 0))
        pos = pos + 4
    end

    local slot_granting_flag = bits_to_uint(bits, pos, 1)
    pos = pos + 1
    if slot_granting_flag == 1 then
        local slot_grant = bits_to_uint(bits, pos, 8)
        add_text(mr_tree, range, string.format("Basic slotgrant: 0x%02X", slot_grant or 0))
        pos = pos + 8
    end

    local chan_alloc_flag = bits_to_uint(bits, pos, 1)
    pos = pos + 1
    if chan_alloc_flag == 1 then
        local chan_alloc_bits = bits_slice(bits, pos, #bits - pos)
        add_text(mr_tree, range, "Channel allocation element: " .. (chan_alloc_bits or "<truncated>"))
        pos = #bits
    end

    if pos < #bits then
        local tm_sdu = bits_slice(bits, pos, #bits - pos)
        if tm_sdu ~= nil and tm_sdu ~= "" then
            if encryption_mode == 0 then
                parse_tm_sdu(tm_sdu, mr_tree, range, direction, "TM-SDU")
            else
                add_text(mr_tree, range, "Encrypted TM-SDU bits: " .. preview_bits(tm_sdu, 160))
            end
        end
    end
end

local function parse_mac_data(bits, tree, range, direction)
    local fill_bits = bits_to_uint(bits, 2, 1)
    local encrypted = bits_to_uint(bits, 3, 1)
    local addr_type = bits_to_uint(bits, 4, 2)
    if fill_bits == nil or addr_type == nil then
        add_text(tree, range, "MAC-DATA truncated")
        return
    end

    local md_tree = tree:add(range, "MAC-DATA")
    add_text(md_tree, range, string.format("fill_bits=%s encrypted=%s addr_type=%u",
        fill_bits == 1 and "true" or "false",
        encrypted == 1 and "true" or "false",
        addr_type
    ))

    local pos = 6
    if addr_type == 0 or addr_type == 2 or addr_type == 3 then
        local addr = bits_to_uint(bits, pos, 24)
        add_text(md_tree, range, string.format("Address: %u", addr or 0))
        pos = pos + 24
    elseif addr_type == 1 then
        local event_label = bits_to_uint(bits, pos, 10)
        add_text(md_tree, range, string.format("Event label: %u", event_label or 0))
        pos = pos + 10
    end

    local length_ind_or_cap_req = bits_to_uint(bits, pos, 1)
    pos = pos + 1
    if length_ind_or_cap_req == 0 then
        local length_ind = bits_to_uint(bits, pos, 6)
        add_text(md_tree, range, string.format("Length indication: %u", length_ind or 0))
        pos = pos + 6
    else
        local frag_flag = bits_to_uint(bits, pos, 1)
        local reservation_req = bits_to_uint(bits, pos + 1, 4)
        add_text(md_tree, range, string.format("Fragmentation: frag_flag=%s reservation_requirement=%s",
            frag_flag == 1 and "true" or "false",
            lookup(RES_REQ_NAMES, reservation_req, tostring(reservation_req))
        ))
        pos = pos + 6
    end

    if pos < #bits then
        local sdu_bits = bits_slice(bits, pos, #bits - pos)
        if sdu_bits ~= nil and sdu_bits ~= "" then
            if encrypted == 0 then
                parse_tm_sdu(sdu_bits, md_tree, range, direction, "TM-SDU")
            else
                add_text(md_tree, range, "Encrypted TM-SDU bits: " .. preview_bits(sdu_bits, 160))
            end
        end
    end
end

local function parse_mac_end_or_frag(bits, tree, range, direction, logical_channel, ctx)
    if logical_channel == 7 then
        local mac_pdu_type = bits_to_uint(bits, 0, 1)
        if mac_pdu_type == 1 then
            local fill_bits = bits_to_uint(bits, 1, 1)
            local length_ind_or_cap_req = bits_to_uint(bits, 2, 1)
            if length_ind_or_cap_req == 0 then
                local length_ind = bits_to_uint(bits, 3, 4)
                add_text(tree, range, string.format("MAC-END-HU: fill_bits=%s length_ind=%u",
                    fill_bits == 1 and "true" or "false",
                    length_ind or 0
                ))
                if length_ind ~= nil then
                    local pdu_len_bits = length_ind * 8
                    local frag_bits = extract_mac_payload(bits, 7, pdu_len_bits, fill_bits)
                    if frag_bits ~= nil and frag_bits ~= "" then
                        local pending = pop_sch_hu_fragment(ctx)
                        if pending ~= nil and pending.bits ~= nil then
                            local combined = pending.bits .. frag_bits
                            local label = "Reassembled TM-SDU"
                            if pending.addr ~= nil then
                                label = label .. " (addr " .. pending.addr .. ")"
                            end
                            parse_tm_sdu(combined, tree, range, direction, label)
                        else
                            parse_tm_sdu(frag_bits, tree, range, direction, "TM-SDU")
                        end
                    end
                end
            else
                local reservation_req = bits_to_uint(bits, 3, 4)
                add_text(tree, range, string.format("MAC-END-HU: fill_bits=%s reservation_requirement=%s",
                    fill_bits == 1 and "true" or "false",
                    lookup(RES_REQ_NAMES, reservation_req, tostring(reservation_req))
                ))
            end
        else
            add_text(tree, range, "SCH/HU subtype not decoded; raw bits follow")
        end
        return
    end

    local subtype = bits_to_uint(bits, 2, 1)
    local fill_bits = bits_to_uint(bits, 3, 1)
    if subtype == nil or fill_bits == nil then
        add_text(tree, range, "MAC-END/FRAG truncated")
        return
    end

    if subtype == 0 then
        add_text(tree, range, string.format("MAC-FRAG (%s): fill_bits=%s",
            direction == 0 and "downlink" or "uplink",
            fill_bits == 1 and "true" or "false"
        ))
        if #bits > 4 then
            local sdu_bits = bits_slice(bits, 4, #bits - 4)
            if sdu_bits ~= nil and sdu_bits ~= "" then
                add_text(tree, range, "TM-SDU fragment bits: " .. preview_bits(sdu_bits, 160))
            end
        end
        return
    end

    if direction == 0 then
        local pos_of_grant = bits_to_uint(bits, 4, 1)
        local length_ind = bits_to_uint(bits, 5, 6)
        add_text(tree, range, string.format("MAC-END-DL: fill_bits=%s pos_of_grant=%u length_ind=%u",
            fill_bits == 1 and "true" or "false",
            pos_of_grant or 0,
            length_ind or 0
        ))
    else
        local length_ind_cap_req = bits_to_uint(bits, 4, 6)
        if length_ind_cap_req ~= nil and length_ind_cap_req >= 48 then
            local reservation_req = length_ind_cap_req % 16
            add_text(tree, range, string.format("MAC-END-UL: fill_bits=%s reservation_requirement=%s",
                fill_bits == 1 and "true" or "false",
                lookup(RES_REQ_NAMES, reservation_req, tostring(reservation_req))
            ))
        else
            add_text(tree, range, string.format("MAC-END-UL: fill_bits=%s length_ind=%u",
                fill_bits == 1 and "true" or "false",
                length_ind_cap_req or 0
            ))
        end
    end
end

local function parse_mac_u_signal(bits, tree, range)
    local second_half_stolen = bits_to_uint(bits, 2, 1)
    add_text(tree, range, string.format("MAC-U-SIGNAL: second_half_stolen=%s",
        second_half_stolen == 1 and "true" or "false"
    ))
end

local function parse_mac_u_blck(bits, tree, range)
    local supp_pdu_subtype = bits_to_uint(bits, 2, 1)
    local fill_bits = bits_to_uint(bits, 3, 1)
    local encrypted = bits_to_uint(bits, 4, 1)
    local event_label = bits_to_uint(bits, 5, 10)
    local reservation_req = bits_to_uint(bits, 15, 4)
    if supp_pdu_subtype == nil or fill_bits == nil or encrypted == nil or event_label == nil or reservation_req == nil then
        add_text(tree, range, "MAC-U-BLCK truncated")
        return
    end

    local mub_tree = tree:add(range, "MAC-U-BLCK")
    add_text(mub_tree, range, string.format("fill_bits=%s encrypted=%s event_label=%u reservation_requirement=%s",
        fill_bits == 1 and "true" or "false",
        encrypted == 1 and "true" or "false",
        event_label,
        lookup(RES_REQ_NAMES, reservation_req, tostring(reservation_req))
    ))
end

local function parse_mac_access(bits, tree, range, ctx)
    local fill_bits = bits_to_uint(bits, 1, 1)
    local encrypted = bits_to_uint(bits, 2, 1)
    local addr_type = bits_to_uint(bits, 3, 2)
    if fill_bits == nil or encrypted == nil or addr_type == nil then
        add_text(tree, range, "MAC-ACCESS truncated")
        return
    end

    local ma_tree = tree:add(range, "MAC-ACCESS")
    add_text(ma_tree, range, string.format("fill_bits=%s encrypted=%s addr_type=%u",
        fill_bits == 1 and "true" or "false",
        encrypted == 1 and "true" or "false",
        addr_type
    ))

    local pos = 5
    local addr_text = nil
    if addr_type == 0 or addr_type == 2 or addr_type == 3 then
        local addr = bits_to_uint(bits, pos, 24)
        add_text(ma_tree, range, string.format("Address: %u", addr or 0))
        if addr ~= nil then
            addr_text = tostring(addr)
        end
        pos = pos + 24
    elseif addr_type == 1 then
        local event_label = bits_to_uint(bits, pos, 10)
        add_text(ma_tree, range, string.format("Event label: %u", event_label or 0))
        pos = pos + 10
    end

    local optional_field_flag = bits_to_uint(bits, pos, 1)
    if optional_field_flag == nil then
        add_text(ma_tree, range, "Optional field flag: truncated")
        return
    end
    pos = pos + 1

    local pdu_len_bits = #bits
    if optional_field_flag == 0 then
        add_text(ma_tree, range, "Optional field flag: false")
    else
        add_text(ma_tree, range, "Optional field flag: true")
        local length_ind_or_cap_req = bits_to_uint(bits, pos, 1)
        pos = pos + 1
        if length_ind_or_cap_req == nil then
            add_text(ma_tree, range, "Length indication/capacity request flag: truncated")
            return
        end

        if length_ind_or_cap_req == 0 then
            local length_ind = bits_to_uint(bits, pos, 5)
            add_text(ma_tree, range, string.format("Length indication: %u", length_ind or 0))
            if length_ind == nil then
                return
            end
            pos = pos + 5
            pdu_len_bits = length_ind * 8
            if length_ind == 0 then
                return
            end
        else
            local frag_flag = bits_to_uint(bits, pos, 1)
            local reservation_req = bits_to_uint(bits, pos + 1, 4)
            add_text(ma_tree, range, string.format("Fragmentation: frag_flag=%s reservation_requirement=%s",
                frag_flag == 1 and "true" or "false",
                lookup(RES_REQ_NAMES, reservation_req, tostring(reservation_req))
            ))
            pos = pos + 5
            if frag_flag == 1 then
                local frag_bits = extract_mac_payload(bits, pos, pdu_len_bits, fill_bits)
                if frag_bits ~= nil and frag_bits ~= "" then
                    if not ctx.visited then
                        remember_sch_hu_fragment(ctx, frag_bits, addr_text)
                    end
                    add_text(ma_tree, range, "TM-SDU fragment bits: " .. preview_bits(frag_bits, 160))
                end
                return
            end
        end
    end

    local sdu_bits = extract_mac_payload(bits, pos, pdu_len_bits, fill_bits)
    if sdu_bits ~= nil and sdu_bits ~= "" then
        if encrypted == 0 then
            parse_tm_sdu(sdu_bits, ma_tree, range, 1, "TM-SDU")
        else
            add_text(ma_tree, range, "Encrypted TM-SDU bits: " .. preview_bits(sdu_bits, 160))
        end
    end
end

local function parse_generic_mac(bits, tree, range, direction, logical_channel, ctx)
    if #bits < 2 then
        add_text(tree, range, "MAC block truncated")
        return
    end

    if logical_channel == 7 then
        local sch_hu_type = bits_to_uint(bits, 0, 1)
        if sch_hu_type == 0 then
            parse_mac_access(bits, tree, range, ctx)
        else
            parse_mac_end_or_frag(bits, tree, range, direction, logical_channel, ctx)
        end
        return
    end

    local mac_pdu_type = bits_to_uint(bits, 0, 2)
    local mac_tree = tree:add(range, "MAC PDU: " .. lookup(MAC_PDU_NAMES, mac_pdu_type, "Unknown"))
    if mac_pdu_type == 0 then
        if direction == 0 then
            parse_mac_resource(bits, mac_tree, range, direction)
        else
            parse_mac_data(bits, mac_tree, range, direction)
        end
    elseif mac_pdu_type == 1 then
        parse_mac_end_or_frag(bits, mac_tree, range, direction, logical_channel, ctx)
    elseif mac_pdu_type == 2 then
        local broadcast_type = bits_to_uint(bits, 2, 2)
        add_text(mac_tree, range, "Broadcast subtype: " .. lookup(BROADCAST_NAMES, broadcast_type, "Unknown"))
        if logical_channel == 3 and broadcast_type == 0 then
            parse_bnch(bits, mac_tree, range)
        elseif #bits > 4 then
            local tm_sdu = bits_slice(bits, 4, #bits - 4)
            add_text(mac_tree, range, "Broadcast payload bits: " .. (tm_sdu or "<truncated>"))
        end
    elseif mac_pdu_type == 3 then
        if logical_channel == 6 and direction == 1 then
            parse_mac_u_signal(bits, mac_tree, range)
            if #bits > 3 then
                local tail_bits = bits_slice(bits, 3, #bits - 3)
                if tail_bits ~= nil and tail_bits ~= "" then
                    parse_tm_sdu(tail_bits, mac_tree, range, direction, "TM-SDU")
                end
            end
        else
            local supp_pdu_subtype = bits_to_uint(bits, 2, 1)
            if supp_pdu_subtype == 0 then
                parse_mac_u_blck(bits, mac_tree, range)
            else
                add_text(mac_tree, range, "Supplementary MAC subtype not decoded; raw bits follow")
                add_text(mac_tree, range, "Raw bits: " .. bits)
            end
        end
    else
        add_text(mac_tree, range, "Raw bits: " .. bits)
    end
end

local summarize_mm_uplink_payload

local function summarize_mle_payload(bits, direction)
    if bits == nil or bits == "" then
        return nil
    end

    local protocol = bits_to_uint(bits, 0, 3)
    if protocol == nil then
        return nil
    end

    local payload_bits = bits_slice(bits, 3, #bits - 3) or ""
    local protocol_name = lookup(MLE_PROTOCOL_NAMES, protocol, "Unknown")

    if protocol == 1 then
        local pdu_type = bits_to_uint(payload_bits, 0, 4)
        if pdu_type ~= nil then
            if direction == 0 then
                return lookup(MM_PDU_NAMES, pdu_type, protocol_name)
            end
            return summarize_mm_uplink_payload(payload_bits)
        end
    elseif protocol == 2 then
        local pdu_type = bits_to_uint(payload_bits, 0, 5)
        if pdu_type ~= nil then
            if direction == 0 then
                return lookup(CMCE_PDU_NAMES, pdu_type, protocol_name)
            end
            return lookup(CMCE_PDU_NAMES_UL, pdu_type, protocol_name)
        end
    elseif protocol == 5 and direction == 0 then
        local pdu_type = bits_to_uint(payload_bits, 0, 3)
        if pdu_type ~= nil then
            return lookup(MLE_PDU_NAMES_DL, pdu_type, protocol_name)
        end
    end

    return protocol_name
end

summarize_mm_uplink_payload = function(payload_bits)
    local pdu_type = bits_to_uint(payload_bits, 0, 4)
    if pdu_type == nil then
        return nil
    end

    local summary = lookup(MM_PDU_NAMES_UL, pdu_type, "MM")
    if pdu_type == 2 then
        local location_update_type = bits_to_uint(payload_bits, 4, 3)
        if location_update_type ~= nil then
            return summary .. " / " .. lookup(LOCATION_UPDATE_TYPE_NAMES, location_update_type, tostring(location_update_type))
        end
    elseif pdu_type == 7 or pdu_type == 8 then
        local mode = bits_to_uint(payload_bits, 5, 1)
        if mode ~= nil then
            return summary .. " / " .. lookup(GROUP_IDENTITY_ATTACH_DETACH_MODE_UL_NAMES, mode, tostring(mode))
        end
    end

    return summary
end

local function summarize_tl_sdu(bits, direction)
    return summarize_mle_payload(bits, direction)
end

local function summarize_llc(bits, direction)
    if bits == nil or bits == "" then
        return nil
    end

    local cur = new_cursor(bits)
    local pdu_type = cursor_read_uint(cur, 4)
    if pdu_type == nil then
        return nil
    end

    local summary = lookup(LLC_PDU_NAMES, pdu_type, "LLC")

    if pdu_type >= 8 then
        return summary
    end

    local has_fcs = pdu_type >= 4 and pdu_type <= 7
    local basic_type = pdu_type % 4

    if basic_type == 0 then
        if cursor_read_uint(cur, 1) == nil or cursor_read_uint(cur, 1) == nil then
            return summary
        end
    elseif basic_type == 1 then
        if cursor_read_uint(cur, 1) == nil then
            return summary
        end
    elseif basic_type == 3 then
        return summary
    end

    local remaining = cursor_remaining(cur)
    if remaining < 0 then
        return summary
    end

    local payload_bits = nil
    if has_fcs == 1 then
        if remaining < 32 then
            payload_bits = cursor_read_bits(cur, remaining)
        else
            payload_bits = cursor_read_bits(cur, remaining - 32)
        end
    else
        payload_bits = cursor_read_bits(cur, remaining)
    end

    local inner = summarize_tl_sdu(payload_bits or "", direction)
    if inner ~= nil and inner ~= "" then
        return summary .. " / " .. inner
    end

    return summary
end

local function summarize_mac_resource(bits, direction)
    local encryption_mode = bits_to_uint(bits, 4, 2)
    local addr_type = bits_to_uint(bits, 13, 3)
    if encryption_mode == nil or addr_type == nil then
        return "MAC-RESOURCE"
    end

    local pos = 16
    if addr_type == 1 or addr_type == 3 or addr_type == 4 then
        pos = pos + 24
    elseif addr_type == 2 then
        pos = pos + 10
    elseif addr_type == 5 or addr_type == 7 then
        pos = pos + 34
    elseif addr_type == 6 then
        pos = pos + 30
    else
        return "MAC-RESOURCE"
    end

    local power_control_flag = bits_to_uint(bits, pos, 1)
    pos = pos + 1
    if power_control_flag == 1 then
        pos = pos + 4
    end

    local slot_granting_flag = bits_to_uint(bits, pos, 1)
    pos = pos + 1
    if slot_granting_flag == 1 then
        pos = pos + 8
    end

    local chan_alloc_flag = bits_to_uint(bits, pos, 1)
    pos = pos + 1
    if chan_alloc_flag == 1 then
        return "MAC-RESOURCE / Channel Allocation"
    end

    if encryption_mode ~= 0 then
        return "MAC-RESOURCE / Encrypted"
    end

    local tm_sdu = bits_slice(bits, pos, #bits - pos)
    local inner = summarize_llc(tm_sdu, direction)
    if inner ~= nil and inner ~= "" then
        return "MAC-RESOURCE / " .. inner
    end

    return "MAC-RESOURCE"
end

local function summarize_mac_access(bits, direction, ctx)
    local fill_bits = bits_to_uint(bits, 1, 1)
    local encrypted = bits_to_uint(bits, 2, 1)
    local addr_type = bits_to_uint(bits, 3, 2)
    if fill_bits == nil or encrypted == nil or addr_type == nil then
        return "MAC-ACCESS"
    end

    local pos = 5
    if addr_type == 0 or addr_type == 2 or addr_type == 3 then
        pos = pos + 24
    elseif addr_type == 1 then
        pos = pos + 10
    end

    local optional_field_flag = bits_to_uint(bits, pos, 1)
    if optional_field_flag == nil then
        return "MAC-ACCESS"
    end

    local prefix = "MAC-ACCESS"
    local pdu_len_bits = #bits
    pos = pos + 1
    if optional_field_flag == 1 then
        local length_ind_or_cap_req = bits_to_uint(bits, pos, 1)
        pos = pos + 1
        if length_ind_or_cap_req == nil then
            return prefix
        end

        if length_ind_or_cap_req == 0 then
            local length_ind = bits_to_uint(bits, pos, 5)
            if length_ind == nil then
                return prefix
            end
            pos = pos + 5
            pdu_len_bits = length_ind * 8
            if length_ind == 0 then
                return prefix .. " / Null"
            elseif length_ind == 31 then
                return prefix .. " / FragmentStart"
            end
        else
            local frag_flag = bits_to_uint(bits, pos, 1)
            local reservation_req = bits_to_uint(bits, pos + 1, 4)
            pos = pos + 5
            if reservation_req ~= nil then
                prefix = prefix .. " / " .. lookup(RES_REQ_NAMES, reservation_req, "CapacityRequest")
            else
                prefix = prefix .. " / CapacityRequest"
            end
            if frag_flag == 1 then
                local frag_bits = extract_mac_payload(bits, pos, pdu_len_bits, fill_bits)
                if not ctx.visited then
                    remember_sch_hu_summary_fragment(ctx, frag_bits, nil)
                end
                return prefix .. " / FragmentStart"
            end
        end
    end

    if encrypted == 1 then
        return prefix .. " / Encrypted"
    end

    local tm_sdu = extract_mac_payload(bits, pos, pdu_len_bits, fill_bits)
    local inner = summarize_llc(tm_sdu, direction)
    if inner ~= nil and inner ~= "" then
        return prefix .. " / " .. inner
    end

    return prefix
end

local function summarize_mac_end_hu(bits, direction, ctx)
    local fill_bits = bits_to_uint(bits, 1, 1)
    local length_ind_or_cap_req = bits_to_uint(bits, 2, 1)
    if fill_bits == nil or length_ind_or_cap_req == nil then
        return "MAC-END-HU"
    end
    if length_ind_or_cap_req == 1 then
        local reservation_req = bits_to_uint(bits, 3, 4)
        if reservation_req ~= nil then
            return "MAC-END-HU / " .. lookup(RES_REQ_NAMES, reservation_req, "CapacityRequest")
        end
        return "MAC-END-HU"
    end

    local length_ind = bits_to_uint(bits, 3, 4)
    if length_ind == nil then
        return "MAC-END-HU"
    end
    local frag_bits = extract_mac_payload(bits, 7, length_ind * 8, fill_bits)
    local pending = get_sch_hu_summary_fragment(ctx)
    if pending ~= nil and pending.bits ~= nil and frag_bits ~= nil then
        local inner = summarize_llc(pending.bits .. frag_bits, direction)
        if inner ~= nil and inner ~= "" then
            return "MAC-END-HU / " .. inner
        end
    end
    if frag_bits ~= nil and frag_bits ~= "" then
        local inner = summarize_llc(frag_bits, direction)
        if inner ~= nil and inner ~= "" then
            return "MAC-END-HU / " .. inner
        end
    end
    return "MAC-END-HU"
end

local function summarize_mac_data(bits, direction)
    local encrypted = bits_to_uint(bits, 3, 1)
    local addr_type = bits_to_uint(bits, 4, 2)
    if encrypted == nil or addr_type == nil then
        return "MAC-DATA"
    end

    local pos = 6
    if addr_type == 0 or addr_type == 2 or addr_type == 3 then
        pos = pos + 24
    elseif addr_type == 1 then
        pos = pos + 10
    end

    local length_ind_or_cap_req = bits_to_uint(bits, pos, 1)
    pos = pos + 1
    if length_ind_or_cap_req == nil then
        return "MAC-DATA"
    end
    pos = pos + 6

    if encrypted == 1 then
        return "MAC-DATA / Encrypted"
    end

    local tm_sdu = bits_slice(bits, pos, #bits - pos)
    local inner = summarize_llc(tm_sdu, direction)
    if inner ~= nil and inner ~= "" then
        return "MAC-DATA / " .. inner
    end

    return "MAC-DATA"
end

local function summarize_mac(bits, direction, logical_channel, ctx)
    if bits == nil or bits == "" then
        return nil
    end

    if logical_channel == 7 then
        local sch_hu_type = bits_to_uint(bits, 0, 1)
        if sch_hu_type == 0 then
            return summarize_mac_access(bits, direction, ctx)
        end
        return summarize_mac_end_hu(bits, direction, ctx)
    end

    local mac_pdu_type = bits_to_uint(bits, 0, 2)
    if mac_pdu_type == nil then
        return nil
    end

    if mac_pdu_type == 0 then
        if direction == 0 then
            return summarize_mac_resource(bits, direction)
        end
        return summarize_mac_data(bits, direction)
    elseif mac_pdu_type == 1 then
        if logical_channel == 7 then
            return "MAC-END-HU"
        elseif direction == 0 then
            return "MAC-END-DL"
        end
        return "MAC-END-UL"
    elseif mac_pdu_type == 2 then
        local broadcast_type = bits_to_uint(bits, 2, 2)
        if broadcast_type ~= nil then
            return lookup(BROADCAST_NAMES, broadcast_type, "Broadcast")
        end
        return "Broadcast"
    elseif mac_pdu_type == 3 then
        if logical_channel == 6 and direction == 1 then
            local tail_bits = bits_slice(bits, 3, #bits - 3)
            local inner = summarize_llc(tail_bits, direction)
            if inner ~= nil and inner ~= "" then
                return "MAC-U-SIGNAL / " .. inner
            end
            return "MAC-U-SIGNAL"
        end
        local supp_pdu_subtype = bits_to_uint(bits, 2, 1)
        if supp_pdu_subtype == 0 then
            return "MAC-U-BLCK"
        end
        return "Supplementary"
    end

    return lookup(MAC_PDU_NAMES, mac_pdu_type, "MAC")
end

local function summarize_type1(bits, direction, logical_channel, frame, ctx)
    if logical_channel == 1 then
        return frame == 18 and "ACCESS-ASSIGN FR18" or "ACCESS-ASSIGN"
    elseif logical_channel == 2 then
        return "MAC-SYNC / D-MLE-SYNC"
    elseif logical_channel == 3 then
        return "MAC-SYSINFO / D-MLE-SYSINFO"
    elseif logical_channel == 4 or logical_channel == 5 or logical_channel == 6 or logical_channel == 7 then
        return summarize_mac(bits, direction, logical_channel, ctx)
    elseif logical_channel >= 8 and logical_channel <= 11 then
        return "Traffic payload"
    end

    return nil
end

local function decode_type1(bits, tree, range, direction, logical_channel, frame, crc_pass, ctx)
    if crc_pass ~= 1 and logical_channel ~= 1 then
        add_text(tree, range, "Control-block CRC failed in BlueStation; payload decode is unreliable, so no deeper MAC/LLC/MM/CMCE dissection is attempted")
        return
    end

    if logical_channel == 1 then
        parse_access_assign(bits, tree, range, frame == 18)
    elseif logical_channel == 2 then
        parse_bsch(bits, tree, range)
    elseif logical_channel == 3 then
        parse_bnch(bits, tree, range)
    elseif logical_channel == 4 or logical_channel == 5 or logical_channel == 6 or logical_channel == 7 then
        parse_generic_mac(bits, tree, range, direction, logical_channel, ctx)
    elseif logical_channel >= 8 and logical_channel <= 11 then
        add_text(tree, range, "Traffic channel type-1 bits: voice/data payload not further dissected")
    else
        add_text(tree, range, "Logical channel not dissected; raw type-1 bits preserved")
    end
end

local function dissect_impl(tvb, pinfo, tree)
    if tvb:len() < 21 then
        return false
    end
    if tvb(0, 4):string() ~= "TBSW" then
        return false
    end

    local version = tvb(4, 1):uint()
    local direction = tvb(5, 1):uint()
    local logical_channel = tvb(6, 1):uint()
    local block = tvb(7, 1):uint()
    local crc_pass = tvb(8, 1):uint()
    local hyperframe = tvb(10, 2):uint()
    local multiframe = tvb(12, 1):uint()
    local frame = tvb(13, 1):uint()
    local timeslot = tvb(14, 1):uint()
    local bit_length = tvb(19, 2):uint()

    local available_bit_bytes = tvb:len() - 21
    local bit_bytes = math.min(bit_length, available_bit_bytes)
    local payload_range = tvb(21, bit_bytes)
    local bits = payload_range:string()
    local ctx = {
        direction = direction,
        logical_channel = logical_channel,
        timeslot = timeslot,
        packet_number = pinfo.number,
        visited = pinfo.visited,
    }

    pinfo.cols.protocol = "BST1"
    local crc_text
    if is_traffic_channel(logical_channel) then
        crc_text = crc_pass == 1 and "Class2 CRC ok" or "BFI"
    else
        crc_text = crc_pass == 1 and "CRC ok" or "CRC fail"
    end
    local info = string.format("%s %s %s %s",
        lookup(DIRECTION_NAMES, direction, "Unknown"),
        lookup(LOGICAL_CHANNEL_NAMES, logical_channel, "Unknown"),
        lookup(BLOCK_NAMES, block, "Unknown"),
        crc_text
    )
    local type1_summary = nil
    if crc_pass == 1 or logical_channel == 1 then
        type1_summary = summarize_type1(bits, direction, logical_channel, frame, ctx)
    end
    if type1_summary ~= nil and type1_summary ~= "" then
        info = info .. " " .. type1_summary
    end
    pinfo.cols.info = info

    local subtree = tree:add(bluestation_type1, tvb(), "BlueStation Type-1 Capture")
    subtree:add(f.magic, tvb(0, 4))
    subtree:add(f.version, tvb(4, 1))
    subtree:add(f.direction, tvb(5, 1))
    subtree:add(f.logical_channel, tvb(6, 1))
    subtree:add(f.block, tvb(7, 1))
    subtree:add(f.crc_pass, tvb(8, 1))
    subtree:add(f.reserved, tvb(9, 1))
    subtree:add(f.hyperframe, tvb(10, 2))
    subtree:add(f.multiframe, tvb(12, 1))
    subtree:add(f.frame, tvb(13, 1))
    subtree:add(f.timeslot, tvb(14, 1))
    subtree:add(f.scrambling_code, tvb(15, 4))
    subtree:add(f.bit_length, tvb(19, 2))
    subtree:add(f.bits, payload_range)

    if is_traffic_channel(logical_channel) then
        add_text(subtree, payload_range, crc_pass == 1
            and "Traffic decode status: speech Class-2 CRC passed"
            or "Traffic decode status: Bad Frame Indication (speech Class-2 CRC failed)")
    end

    if bit_bytes < bit_length then
        add_text(subtree, payload_range, string.format("Truncated capture payload: expected %u bit characters, got %u", bit_length, bit_bytes))
    end

    decode_type1(bits, subtree, payload_range, direction, logical_channel, frame, crc_pass, ctx)
    return true
end

function bluestation_type1.dissector(tvb, pinfo, tree)
    if dissect_impl(tvb, pinfo, tree) then
        return tvb:len()
    end
    return 0
end

bluestation_type1:register_heuristic("udp", dissect_impl)
DissectorTable.get("udp.port"):add(42069, bluestation_type1)
