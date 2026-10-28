# UniFi Network (unifi-network): one vector per command, generated from the
# UniFi Network API 11.0.97 reference (OpenAPI): each operation's method and
# path under /proxy/network/integration on the console, with the site from the
# site_id setting and query values in the order the reference lists them.
UN = "unifi-network"
UN_SETTINGS = {"site_id": "0e7c5b6a-1f2d-4c3b-8a9e-112233445566", "token": "k"}


def _un(command, input, method, target, body=None, body_hex=None, content_type=None):
    request = {"method": method, "target": target}
    if body is not None:
        request["body"] = body
    if body_hex is not None:
        request["body_hex"] = body_hex
    if content_type is not None:
        request["content_type"] = content_type
    V.append({"spec": UN, "command": command, "input": input, "expect_request": request, "settings": UN_SETTINGS})


_un('list_countries', {'offset': 0, 'limit': 25}, 'GET', '/proxy/network/integration/v1/countries?offset=0&limit=25')
_un('list_countries_filtered', {'offset': 0, 'limit': 25, 'filter': "name.eq('Studio')"}, 'GET', '/proxy/network/integration/v1/countries?offset=0&limit=25&filter=name.eq%28%27Studio%27%29')
_un('list_dpi_applications', {'offset': 0, 'limit': 25}, 'GET', '/proxy/network/integration/v1/dpi/applications?offset=0&limit=25')
_un('list_dpi_applications_filtered', {'offset': 0, 'limit': 25, 'filter': "name.eq('Studio')"}, 'GET', '/proxy/network/integration/v1/dpi/applications?offset=0&limit=25&filter=name.eq%28%27Studio%27%29')
_un('list_dpi_application_categories', {'offset': 0, 'limit': 25}, 'GET', '/proxy/network/integration/v1/dpi/categories?offset=0&limit=25')
_un('list_dpi_application_categories_filtered', {'offset': 0, 'limit': 25, 'filter': "name.eq('Studio')"}, 'GET', '/proxy/network/integration/v1/dpi/categories?offset=0&limit=25&filter=name.eq%28%27Studio%27%29')
_un('get_application_info', {}, 'GET', '/proxy/network/integration/v1/info')
_un('list_pending_devices', {'offset': 0, 'limit': 25}, 'GET', '/proxy/network/integration/v1/pending-devices?offset=0&limit=25')
_un('list_pending_devices_filtered', {'offset': 0, 'limit': 25, 'filter': "name.eq('Studio')"}, 'GET', '/proxy/network/integration/v1/pending-devices?offset=0&limit=25&filter=name.eq%28%27Studio%27%29')
_un('list_sites', {'offset': 0, 'limit': 25}, 'GET', '/proxy/network/integration/v1/sites?offset=0&limit=25')
_un('list_sites_filtered', {'offset': 0, 'limit': 25, 'filter': "name.eq('Studio')"}, 'GET', '/proxy/network/integration/v1/sites?offset=0&limit=25&filter=name.eq%28%27Studio%27%29')
_un('list_acl_rules', {'offset': 0, 'limit': 25}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/acl-rules?offset=0&limit=25')
_un('list_acl_rules_filtered', {'offset': 0, 'limit': 25, 'filter': "name.eq('Studio')"}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/acl-rules?offset=0&limit=25&filter=name.eq%28%27Studio%27%29')
_un('create_acl_rule', {'body': {'name': 'Studio'}}, 'POST', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/acl-rules', body='{"name":"Studio"}')
_un('get_user_defined_acl_rule_ordering', {}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/acl-rules/ordering')
_un('reorder_user_defined_acl_rules', {'body': {'name': 'Studio'}}, 'PUT', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/acl-rules/ordering', body='{"name":"Studio"}')
_un('delete_acl_rule', {'acl_rule_id': 'a1b2c3d4-0000-4000-8000-000000000001'}, 'DELETE', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/acl-rules/a1b2c3d4-0000-4000-8000-000000000001')
_un('get_acl_rule', {'acl_rule_id': 'a1b2c3d4-0000-4000-8000-000000000001'}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/acl-rules/a1b2c3d4-0000-4000-8000-000000000001')
_un('update_acl_rule', {'acl_rule_id': 'a1b2c3d4-0000-4000-8000-000000000001', 'body': {'name': 'Studio'}}, 'PUT', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/acl-rules/a1b2c3d4-0000-4000-8000-000000000001', body='{"name":"Studio"}')
_un('list_clients', {'offset': 0, 'limit': 25}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/clients?offset=0&limit=25')
_un('list_clients_filtered', {'offset': 0, 'limit': 25, 'filter': "name.eq('Studio')"}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/clients?offset=0&limit=25&filter=name.eq%28%27Studio%27%29')
_un('get_client', {'client_id': 'a1b2c3d4-0000-4000-8000-000000000001'}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/clients/a1b2c3d4-0000-4000-8000-000000000001')
_un('client_action', {'client_id': 'a1b2c3d4-0000-4000-8000-000000000001', 'body': {'name': 'Studio'}}, 'POST', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/clients/a1b2c3d4-0000-4000-8000-000000000001/actions', body='{"name":"Studio"}')
_un('list_device_tags', {'offset': 0, 'limit': 25}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/device-tags?offset=0&limit=25')
_un('list_device_tags_filtered', {'offset': 0, 'limit': 25, 'filter': "name.eq('Studio')"}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/device-tags?offset=0&limit=25&filter=name.eq%28%27Studio%27%29')
_un('list_devices', {'offset': 0, 'limit': 25}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/devices?offset=0&limit=25')
_un('list_devices_filtered', {'offset': 0, 'limit': 25, 'filter': "name.eq('Studio')"}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/devices?offset=0&limit=25&filter=name.eq%28%27Studio%27%29')
_un('adopt_devices', {'body': {'name': 'Studio'}}, 'POST', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/devices', body='{"name":"Studio"}')
_un('remove_device', {'device_id': 'a1b2c3d4-0000-4000-8000-000000000001'}, 'DELETE', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/devices/a1b2c3d4-0000-4000-8000-000000000001')
_un('get_device', {'device_id': 'a1b2c3d4-0000-4000-8000-000000000001'}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/devices/a1b2c3d4-0000-4000-8000-000000000001')
_un('restart_device', {'device_id': 'a1b2c3d4-0000-4000-8000-000000000001'}, 'POST', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/devices/a1b2c3d4-0000-4000-8000-000000000001/actions', body='{"action":"RESTART"}')
_un('power_cycle_port', {'port_idx': 1, 'device_id': 'a1b2c3d4-0000-4000-8000-000000000001'}, 'POST', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/devices/a1b2c3d4-0000-4000-8000-000000000001/interfaces/ports/1/actions', body='{"action":"POWER_CYCLE"}')
_un('get_device_statistics', {'device_id': 'a1b2c3d4-0000-4000-8000-000000000001'}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/devices/a1b2c3d4-0000-4000-8000-000000000001/statistics/latest')
_un('list_dns_policies', {'offset': 0, 'limit': 25}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/dns/policies?offset=0&limit=25')
_un('list_dns_policies_filtered', {'offset': 0, 'limit': 25, 'filter': "name.eq('Studio')"}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/dns/policies?offset=0&limit=25&filter=name.eq%28%27Studio%27%29')
_un('create_dns_policy', {'body': {'name': 'Studio'}}, 'POST', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/dns/policies', body='{"name":"Studio"}')
_un('delete_dns_policy', {'dns_policy_id': 'a1b2c3d4-0000-4000-8000-000000000001'}, 'DELETE', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/dns/policies/a1b2c3d4-0000-4000-8000-000000000001')
_un('get_dns_policy', {'dns_policy_id': 'a1b2c3d4-0000-4000-8000-000000000001'}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/dns/policies/a1b2c3d4-0000-4000-8000-000000000001')
_un('update_dns_policy', {'dns_policy_id': 'a1b2c3d4-0000-4000-8000-000000000001', 'body': {'name': 'Studio'}}, 'PUT', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/dns/policies/a1b2c3d4-0000-4000-8000-000000000001', body='{"name":"Studio"}')
_un('list_firewall_policies', {'offset': 0, 'limit': 25}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/firewall/policies?offset=0&limit=25')
_un('list_firewall_policies_filtered', {'offset': 0, 'limit': 25, 'filter': "name.eq('Studio')"}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/firewall/policies?offset=0&limit=25&filter=name.eq%28%27Studio%27%29')
_un('create_firewall_policy', {'body': {'name': 'Studio'}}, 'POST', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/firewall/policies', body='{"name":"Studio"}')
_un('get_user_defined_firewall_policy_ordering', {'source_firewall_zone_id': 'a1b2c3d4-0000-4000-8000-000000000001', 'destination_firewall_zone_id': 'a1b2c3d4-0000-4000-8000-000000000001'}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/firewall/policies/ordering?sourceFirewallZoneId=a1b2c3d4-0000-4000-8000-000000000001&destinationFirewallZoneId=a1b2c3d4-0000-4000-8000-000000000001')
_un('reorder_user_defined_firewall_policies', {'source_firewall_zone_id': 'a1b2c3d4-0000-4000-8000-000000000001', 'destination_firewall_zone_id': 'a1b2c3d4-0000-4000-8000-000000000001', 'body': {'name': 'Studio'}}, 'PUT', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/firewall/policies/ordering?sourceFirewallZoneId=a1b2c3d4-0000-4000-8000-000000000001&destinationFirewallZoneId=a1b2c3d4-0000-4000-8000-000000000001', body='{"name":"Studio"}')
_un('delete_firewall_policy', {'firewall_policy_id': 'a1b2c3d4-0000-4000-8000-000000000001'}, 'DELETE', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/firewall/policies/a1b2c3d4-0000-4000-8000-000000000001')
_un('get_firewall_policy', {'firewall_policy_id': 'a1b2c3d4-0000-4000-8000-000000000001'}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/firewall/policies/a1b2c3d4-0000-4000-8000-000000000001')
_un('patch_firewall_policy', {'firewall_policy_id': 'a1b2c3d4-0000-4000-8000-000000000001', 'body': {'name': 'Studio'}}, 'PATCH', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/firewall/policies/a1b2c3d4-0000-4000-8000-000000000001', body='{"name":"Studio"}')
_un('update_firewall_policy', {'firewall_policy_id': 'a1b2c3d4-0000-4000-8000-000000000001', 'body': {'name': 'Studio'}}, 'PUT', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/firewall/policies/a1b2c3d4-0000-4000-8000-000000000001', body='{"name":"Studio"}')
_un('list_firewall_zones', {'offset': 0, 'limit': 25}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/firewall/zones?offset=0&limit=25')
_un('list_firewall_zones_filtered', {'offset': 0, 'limit': 25, 'filter': "name.eq('Studio')"}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/firewall/zones?offset=0&limit=25&filter=name.eq%28%27Studio%27%29')
_un('create_custom_firewall_zone', {'body': {'name': 'Studio'}}, 'POST', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/firewall/zones', body='{"name":"Studio"}')
_un('delete_custom_firewall_zone', {'firewall_zone_id': 'a1b2c3d4-0000-4000-8000-000000000001'}, 'DELETE', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/firewall/zones/a1b2c3d4-0000-4000-8000-000000000001')
_un('get_firewall_zone', {'firewall_zone_id': 'a1b2c3d4-0000-4000-8000-000000000001'}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/firewall/zones/a1b2c3d4-0000-4000-8000-000000000001')
_un('update_firewall_zone', {'firewall_zone_id': 'a1b2c3d4-0000-4000-8000-000000000001', 'body': {'name': 'Studio'}}, 'PUT', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/firewall/zones/a1b2c3d4-0000-4000-8000-000000000001', body='{"name":"Studio"}')
_un('delete_vouchers', {'filter': "name.eq('Studio')"}, 'DELETE', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/hotspot/vouchers?filter=name.eq%28%27Studio%27%29')
_un('list_vouchers', {'offset': 0, 'limit': 100}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/hotspot/vouchers?offset=0&limit=100')
_un('list_vouchers_filtered', {'offset': 0, 'limit': 100, 'filter': "name.eq('Studio')"}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/hotspot/vouchers?offset=0&limit=100&filter=name.eq%28%27Studio%27%29')
_un('generate_vouchers', {'body': {'name': 'Studio'}}, 'POST', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/hotspot/vouchers', body='{"name":"Studio"}')
_un('delete_voucher', {'voucher_id': 'a1b2c3d4-0000-4000-8000-000000000001'}, 'DELETE', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/hotspot/vouchers/a1b2c3d4-0000-4000-8000-000000000001')
_un('get_voucher_details', {'voucher_id': 'a1b2c3d4-0000-4000-8000-000000000001'}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/hotspot/vouchers/a1b2c3d4-0000-4000-8000-000000000001')
_un('list_networks', {'offset': 0, 'limit': 25}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/networks?offset=0&limit=25')
_un('list_networks_filtered', {'offset': 0, 'limit': 25, 'filter': "name.eq('Studio')"}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/networks?offset=0&limit=25&filter=name.eq%28%27Studio%27%29')
_un('create_network', {'body': {'name': 'Studio'}}, 'POST', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/networks', body='{"name":"Studio"}')
_un('delete_network', {'network_id': 'a1b2c3d4-0000-4000-8000-000000000001', 'force': False}, 'DELETE', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/networks/a1b2c3d4-0000-4000-8000-000000000001?force=false')
_un('get_network_details', {'network_id': 'a1b2c3d4-0000-4000-8000-000000000001'}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/networks/a1b2c3d4-0000-4000-8000-000000000001')
_un('update_network', {'network_id': 'a1b2c3d4-0000-4000-8000-000000000001', 'body': {'name': 'Studio'}}, 'PUT', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/networks/a1b2c3d4-0000-4000-8000-000000000001', body='{"name":"Studio"}')
_un('get_network_references', {'network_id': 'a1b2c3d4-0000-4000-8000-000000000001'}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/networks/a1b2c3d4-0000-4000-8000-000000000001/references')
_un('list_radius_profiles', {'offset': 0, 'limit': 25}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/radius/profiles?offset=0&limit=25')
_un('list_radius_profiles_filtered', {'offset': 0, 'limit': 25, 'filter': "name.eq('Studio')"}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/radius/profiles?offset=0&limit=25&filter=name.eq%28%27Studio%27%29')
_un('list_lags', {'offset': 0, 'limit': 25}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/switching/lags?offset=0&limit=25')
_un('list_lags_filtered', {'offset': 0, 'limit': 25, 'filter': "name.eq('Studio')"}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/switching/lags?offset=0&limit=25&filter=name.eq%28%27Studio%27%29')
_un('get_lag', {'lag_id': 'a1b2c3d4-0000-4000-8000-000000000001'}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/switching/lags/a1b2c3d4-0000-4000-8000-000000000001')
_un('list_mc_lag_domains', {'offset': 0, 'limit': 25}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/switching/mc-lag-domains?offset=0&limit=25')
_un('list_mc_lag_domains_filtered', {'offset': 0, 'limit': 25, 'filter': "name.eq('Studio')"}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/switching/mc-lag-domains?offset=0&limit=25&filter=name.eq%28%27Studio%27%29')
_un('get_mc_lag_domain', {'mc_lag_domain_id': 'a1b2c3d4-0000-4000-8000-000000000001'}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/switching/mc-lag-domains/a1b2c3d4-0000-4000-8000-000000000001')
_un('list_switch_stacks', {'offset': 0, 'limit': 25}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/switching/switch-stacks?offset=0&limit=25')
_un('list_switch_stacks_filtered', {'offset': 0, 'limit': 25, 'filter': "name.eq('Studio')"}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/switching/switch-stacks?offset=0&limit=25&filter=name.eq%28%27Studio%27%29')
_un('get_switch_stack', {'switch_stack_id': 'a1b2c3d4-0000-4000-8000-000000000001'}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/switching/switch-stacks/a1b2c3d4-0000-4000-8000-000000000001')
_un('list_traffic_matching_lists', {'offset': 0, 'limit': 25}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/traffic-matching-lists?offset=0&limit=25')
_un('list_traffic_matching_lists_filtered', {'offset': 0, 'limit': 25, 'filter': "name.eq('Studio')"}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/traffic-matching-lists?offset=0&limit=25&filter=name.eq%28%27Studio%27%29')
_un('create_traffic_matching_list', {'body': {'name': 'Studio'}}, 'POST', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/traffic-matching-lists', body='{"name":"Studio"}')
_un('delete_traffic_matching_list', {'traffic_matching_list_id': 'a1b2c3d4-0000-4000-8000-000000000001'}, 'DELETE', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/traffic-matching-lists/a1b2c3d4-0000-4000-8000-000000000001')
_un('get_traffic_matching_list', {'traffic_matching_list_id': 'a1b2c3d4-0000-4000-8000-000000000001'}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/traffic-matching-lists/a1b2c3d4-0000-4000-8000-000000000001')
_un('update_traffic_matching_list', {'traffic_matching_list_id': 'a1b2c3d4-0000-4000-8000-000000000001', 'body': {'name': 'Studio'}}, 'PUT', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/traffic-matching-lists/a1b2c3d4-0000-4000-8000-000000000001', body='{"name":"Studio"}')
_un('list_vpn_servers', {'offset': 0, 'limit': 25}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/vpn/servers?offset=0&limit=25')
_un('list_vpn_servers_filtered', {'offset': 0, 'limit': 25, 'filter': "name.eq('Studio')"}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/vpn/servers?offset=0&limit=25&filter=name.eq%28%27Studio%27%29')
_un('list_site_to_site_vpn_tunnels', {'offset': 0, 'limit': 25}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/vpn/site-to-site-tunnels?offset=0&limit=25')
_un('list_site_to_site_vpn_tunnels_filtered', {'offset': 0, 'limit': 25, 'filter': "name.eq('Studio')"}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/vpn/site-to-site-tunnels?offset=0&limit=25&filter=name.eq%28%27Studio%27%29')
_un('list_wan_interfaces', {'offset': 0, 'limit': 25}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/wans?offset=0&limit=25')
_un('list_wifi_broadcasts', {'offset': 0, 'limit': 25}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/wifi/broadcasts?offset=0&limit=25')
_un('list_wifi_broadcasts_filtered', {'offset': 0, 'limit': 25, 'filter': "name.eq('Studio')"}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/wifi/broadcasts?offset=0&limit=25&filter=name.eq%28%27Studio%27%29')
_un('create_wifi_broadcast', {'body': {'name': 'Studio'}}, 'POST', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/wifi/broadcasts', body='{"name":"Studio"}')
_un('delete_wifi_broadcast', {'wifi_broadcast_id': 'a1b2c3d4-0000-4000-8000-000000000001', 'force': False}, 'DELETE', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/wifi/broadcasts/a1b2c3d4-0000-4000-8000-000000000001?force=false')
_un('get_wifi_broadcast_details', {'wifi_broadcast_id': 'a1b2c3d4-0000-4000-8000-000000000001'}, 'GET', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/wifi/broadcasts/a1b2c3d4-0000-4000-8000-000000000001')
_un('update_wifi_broadcast', {'wifi_broadcast_id': 'a1b2c3d4-0000-4000-8000-000000000001', 'body': {'name': 'Studio'}}, 'PUT', '/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566/wifi/broadcasts/a1b2c3d4-0000-4000-8000-000000000001', body='{"name":"Studio"}')
# Telemetry: the lists polled (offset 0, limit 200), a device's details and
# its latest statistics, shaped as the reference's schemas.
_UNB = "/proxy/network/integration/v1/sites/0e7c5b6a-1f2d-4c3b-8a9e-112233445566"
_DEV = "a1b2c3d4-0000-4000-8000-000000000001"
telemetry(UN, "devices", settings=UN_SETTINGS, inbound_http={
    "path": _UNB + "/devices?offset=0&limit=200",
    "body": '{"offset":0,"limit":200,"count":1,"totalCount":1,"data":[{"id":"' + _DEV + '","macAddress":"94:2a:6f:00:00:01",'
            '"ipAddress":"10.0.1.20","name":"Stage switch","model":"USW Pro 24 PoE","state":"ONLINE","supported":true,'
            '"firmwareVersion":"7.1.26","firmwareUpdatable":false,"features":["switching"],"interfaces":["ports"]}]}'},
    expect_state={"devices": {_DEV: {"mac_address": "94:2a:6f:00:00:01", "ip_address": "10.0.1.20",
                                      "name": "Stage switch", "model": "USW Pro 24 PoE", "state": "ONLINE",
                                      "supported": True, "firmware_version": "7.1.26", "firmware_updatable": False,
                                      "features": '["switching"]', "interfaces": '["ports"]'}}},
    expect_then_send=[{"method": "GET", "target": _UNB + "/devices/" + _DEV},
                      {"method": "GET", "target": _UNB + "/devices/" + _DEV + "/statistics/latest"}])
telemetry(UN, "device-ports", settings=UN_SETTINGS, inbound_http={
    "path": _UNB + "/devices/" + _DEV,
    "body": '{"id":"' + _DEV + '","firmwareVersion":"7.1.26","firmwareUpdatable":false,"configurationId":"c1",'
            '"uplink":{"deviceId":"a1b2c3d4-0000-4000-8000-000000000009"},"features":{"switching":{}},'
            '"interfaces":{"ports":[{"idx":1,"state":"UP","connector":"RJ45","maxSpeedMbps":1000,"speedMbps":1000,'
            '"poe":{"standard":"802.3at","type":2,"enabled":true,"state":"UP"}},'
            '{"idx":25,"state":"DOWN","connector":"SFPPLUS","maxSpeedMbps":10000,"speedMbps":0}]}}'},
    expect_state={"devices": {_DEV: {
        "firmware_version": "7.1.26", "firmware_updatable": False, "configuration_id": "c1",
        "uplink_device_id": "a1b2c3d4-0000-4000-8000-000000000009", "features_detail": '{"switching":{}}',
        "ports": {"1": {"state": "UP", "connector": "RJ45", "speed_mbps": 1000, "max_speed_mbps": 1000,
                        "poe_enabled": True, "poe_state": "UP", "poe_standard": "802.3at", "poe_type": 2},
                  "25": {"state": "DOWN", "connector": "SFPPLUS", "speed_mbps": 0, "max_speed_mbps": 10000}}}}})
telemetry(UN, "device-statistics", settings=UN_SETTINGS, inbound_http={
    "path": _UNB + "/devices/" + _DEV + "/statistics/latest",
    "body": '{"uptimeSec":86400,"lastHeartbeatAt":"2026-10-28T10:00:00Z","nextHeartbeatAt":"2026-10-28T10:00:10Z",'
            '"loadAverage1Min":0.5,"loadAverage5Min":0.4,"loadAverage15Min":0.3,"cpuUtilizationPct":12.5,'
            '"memoryUtilizationPct":40.0,"uplink":{"txRateBps":1200,"rxRateBps":3400}}'},
    expect_state={"devices": {_DEV: {"statistics": {
        "uptime_sec": 86400, "last_heartbeat_at": "2026-10-28T10:00:00Z", "next_heartbeat_at": "2026-10-28T10:00:10Z",
        "load_average_1min": 0.5, "load_average_5min": 0.4, "load_average_15min": 0.3, "cpu_utilization_pct": 12.5,
        "memory_utilization_pct": 40.0, "uplink_tx_rate_bps": 1200, "uplink_rx_rate_bps": 3400}}}})
telemetry(UN, "networks", settings=UN_SETTINGS, inbound_http={
    "path": _UNB + "/networks?offset=0&limit=200",
    "body": '{"offset":0,"limit":200,"count":1,"totalCount":1,"data":[{"id":"b1b2c3d4-0000-4000-8000-000000000002",'
            '"name":"Dante","enabled":true,"vlanId":20,"management":"GATEWAY","default":false,'
            '"metadata":{"origin":"USER_DEFINED"}}]}'},
    expect_state={"networks": {"b1b2c3d4-0000-4000-8000-000000000002": {
        "name": "Dante", "enabled": True, "vlan_id": 20, "management": "GATEWAY", "default": False,
        "metadata": '{"origin":"USER_DEFINED"}'}}})
telemetry(UN, "application-info", settings=UN_SETTINGS,
          inbound_http={"path": "/proxy/network/integration/v1/info", "body": '{"applicationVersion":"9.4.19"}'},
          expect_state={"application": {"version": "9.4.19"}})
